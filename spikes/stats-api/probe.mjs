#!/usr/bin/env node
// Spike: record everything Rocket League's official Stats API emits, per game mode.
//
// Usage:  node probe.mjs [--host 127.0.0.1] [--port 49123]
// While running, type a label + Enter to mark what you're doing
// (e.g. "freeplay", "training", "casual"). Type "q" + Enter (or Ctrl+C) to stop.
// Output: logs/<timestamp>.jsonl (raw events) and logs/<timestamp>-summary.json.

import net from "node:net";
import fs from "node:fs";
import path from "node:path";
import readline from "node:readline";
import { fileURLToPath } from "node:url";

const RECONNECT_MS = 2000;
const SAMPLES_PER_EVENT = 2;

function parseArgs(argv) {
  const opts = { host: "127.0.0.1", port: 49123 };
  for (let i = 0; i < argv.length; i += 2) {
    if (argv[i] === "--host") opts.host = argv[i + 1];
    else if (argv[i] === "--port") opts.port = Number(argv[i + 1]);
    else throw new Error(`Unknown argument: ${argv[i]}`);
  }
  if (!Number.isInteger(opts.port) || opts.port <= 0) throw new Error("Invalid --port");
  return opts;
}

// The TCP stream is a sequence of concatenated JSON objects with no delimiter,
// so split it by tracking brace depth outside of string literals.
export function createJsonFramer(onObject) {
  let buf = "";
  let depth = 0;
  let inString = false;
  let escaped = false;
  let start = -1;

  return (chunk) => {
    const offset = buf.length;
    buf += chunk;
    for (let i = offset; i < buf.length; i++) {
      const c = buf[i];
      if (inString) {
        if (escaped) escaped = false;
        else if (c === "\\") escaped = true;
        else if (c === '"') inString = false;
      } else if (c === '"') inString = true;
      else if (c === "{") {
        if (depth === 0) start = i;
        depth++;
      } else if (c === "}" && depth > 0) {
        depth--;
        if (depth === 0) onObject(buf.slice(start, i + 1));
      }
    }
    if (depth === 0) buf = "";
    else if (start > 0) {
      buf = buf.slice(start);
      start = 0;
    }
  };
}

// Some builds send Data as a JSON-encoded string; normalise to an object.
export function decodeMessage(text) {
  const msg = JSON.parse(text);
  let data = msg.Data;
  if (typeof data === "string") {
    try {
      data = JSON.parse(data);
    } catch {
      /* keep the raw string */
    }
  }
  return { event: msg.Event ?? "<no Event field>", data };
}

// Flatten an object's shape into dotted key paths ("Players[].Boost") with value types.
export function shapeOf(value, prefix = "", out = new Map()) {
  if (Array.isArray(value)) {
    value.forEach((v) => shapeOf(v, `${prefix}[]`, out));
    if (value.length === 0) out.set(`${prefix}[]`, "empty");
  } else if (value && typeof value === "object") {
    for (const [k, v] of Object.entries(value)) shapeOf(v, prefix ? `${prefix}.${k}` : k, out);
  } else {
    out.set(prefix || "<root>", value === null ? "null" : typeof value);
  }
  return out;
}

function createRecorder(logDir) {
  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  fs.mkdirSync(logDir, { recursive: true });
  const rawPath = path.join(logDir, `${stamp}.jsonl`);
  const summaryPath = path.join(logDir, `${stamp}-summary.json`);
  const raw = fs.createWriteStream(rawPath, { flags: "a" });
  const phases = new Map();
  let label = "unlabelled";

  const phase = () => {
    if (!phases.has(label)) phases.set(label, { startedAt: new Date().toISOString(), events: new Map() });
    return phases.get(label);
  };

  return {
    rawPath,
    summaryPath,
    setLabel(next) {
      label = next;
      phase();
      raw.write(JSON.stringify({ t: Date.now(), marker: label }) + "\n");
      console.log(`\n>> now labelling events as "${label}"`);
    },
    record({ event, data }) {
      raw.write(JSON.stringify({ t: Date.now(), phase: label, event, data }) + "\n");
      const events = phase().events;
      const entry = events.get(event) ?? { count: 0, fields: new Map(), samples: [] };
      entry.count++;
      for (const [k, type] of shapeOf(data)) entry.fields.set(k, type);
      if (entry.samples.length < SAMPLES_PER_EVENT) entry.samples.push(data);
      events.set(event, entry);
    },
    liveLine() {
      const events = phases.get(label)?.events ?? new Map();
      return [...events].map(([e, { count }]) => `${e}:${count}`).join("  ") || "(no events yet)";
    },
    async close() {
      const summary = Object.fromEntries(
        [...phases].map(([name, p]) => [
          name,
          {
            startedAt: p.startedAt,
            events: Object.fromEntries(
              [...p.events].map(([e, v]) => [
                e,
                { count: v.count, fields: Object.fromEntries([...v.fields].sort()), samples: v.samples },
              ]),
            ),
          },
        ]),
      );
      fs.writeFileSync(summaryPath, JSON.stringify(summary, null, 2));
      await new Promise((resolve) => raw.end(resolve));
    },
  };
}

function connectLoop({ host, port }, recorder) {
  let socket = null;
  let stopped = false;

  const connect = () => {
    if (stopped) return;
    socket = net.createConnection({ host, port });
    socket.setEncoding("utf8");
    const feed = createJsonFramer((text) => {
      try {
        recorder.record(decodeMessage(text));
      } catch (err) {
        recorder.record({ event: "<parse error>", data: { error: err.message, text: text.slice(0, 500) } });
      }
    });
    socket.on("connect", () => console.log(`\nConnected to ${host}:${port}`));
    socket.on("data", feed);
    socket.on("error", (err) => {
      if (err.code !== "ECONNREFUSED") console.error(`\nSocket error: ${err.message}`);
    });
    socket.on("close", () => {
      if (!stopped) setTimeout(connect, RECONNECT_MS);
    });
  };

  connect();
  return () => {
    stopped = true;
    socket?.destroy();
  };
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const here = path.dirname(fileURLToPath(import.meta.url));
  const recorder = createRecorder(path.join(here, "logs"));
  recorder.setLabel("unlabelled");

  console.log(`Waiting for Rocket League Stats API on ${opts.host}:${opts.port} (retries every ${RECONNECT_MS / 1000}s)`);
  console.log(`Raw log: ${recorder.rawPath}`);
  console.log('Type a label + Enter to mark a phase (e.g. "freeplay"), "q" + Enter to finish.');

  const stopSocket = connectLoop(opts, recorder);
  const ticker = setInterval(() => process.stdout.write(`\r${recorder.liveLine()}`.padEnd(120)), 1000);

  let finishing = false;
  const finish = async () => {
    if (finishing) return;
    finishing = true;
    clearInterval(ticker);
    stopSocket();
    await recorder.close();
    console.log(`\nSummary written to ${recorder.summaryPath}`);
    process.exit(0);
  };

  const rl = readline.createInterface({ input: process.stdin });
  rl.on("line", (line) => {
    const text = line.trim();
    if (text === "q") finish();
    else if (text) recorder.setLabel(text);
  });
  process.on("SIGINT", finish);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  main().catch((err) => {
    console.error(err.message);
    process.exit(1);
  });
}
