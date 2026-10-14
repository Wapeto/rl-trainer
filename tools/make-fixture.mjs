#!/usr/bin/env node
// Turn a raw probe.mjs recording into an anonymized, gzipped test fixture.
// Usage: node tools/make-fixture.mjs <raw.jsonl> <out.jsonl.gz> [--me "<PrimaryId>"]
// Without --me, "me" is the first targeted player outside a replay.
import fs from "node:fs";
import zlib from "node:zlib";

const KEEP_TEAM_NAMES = new Set(["Blue", "Orange", ""]);
const ALLOWED_NAME = /^(me|Me|player\d+|Player\d+|Team|Blue|Orange|)$/;
const ALLOWED_ID = /^(Epic\|(me|player\d+)\|0|Unknown\|0\|0)$/;

function parseArgs(argv) {
  const [input, output, flag, me] = argv;
  if (!input || !output || (flag && flag !== "--me")) {
    throw new Error('Usage: node tools/make-fixture.mjs <raw.jsonl> <out.jsonl.gz> [--me "<PrimaryId>"]');
  }
  return { input, output, me };
}

function detectMe(lines) {
  for (const l of lines) {
    const g = l.data?.Game;
    if (l.event !== "UpdateState" || !g?.bHasTarget || g.bReplay) continue;
    const p = l.data.Players.find((x) => x.Name === g.Target.Name && x.TeamNum === g.Target.TeamNum);
    if (p?.PrimaryId) return p.PrimaryId;
  }
  throw new Error("Could not detect the recording owner; pass --me");
}

function createAliaser(lines, meId) {
  const ids = new Map([[meId, "Epic|me|0"]]);
  const names = new Map();
  for (const l of lines) {
    for (const p of l.data?.Players ?? []) {
      if (p.PrimaryId === meId) names.set(p.Name.toLowerCase(), "me");
    }
  }
  const caseLike = (original, alias) =>
    original[0] !== original[0].toLowerCase() ? alias[0].toUpperCase() + alias.slice(1) : alias;
  return {
    id(id) {
      if (id.startsWith("Unknown")) return id;
      if (!ids.has(id)) ids.set(id, `Epic|player${ids.size}|0`);
      return ids.get(id);
    },
    name(name) {
      if (name === "") return name;
      const key = name.toLowerCase();
      if (!names.has(key)) names.set(key, `player${names.size + 1}`);
      return caseLike(name, names.get(key));
    },
  };
}

function scrub(value, alias) {
  if (Array.isArray(value)) return value.map((v) => scrub(v, alias));
  if (!value || typeof value !== "object") return value;
  const isTeam = "ColorPrimary" in value;
  const out = {};
  for (const [k, v] of Object.entries(value)) {
    if (k === "Loadout") continue;
    if (k === "PrimaryId" && typeof v === "string") out[k] = alias.id(v);
    else if ((k === "Name" || k === "PlayerName") && typeof v === "string")
      out[k] = isTeam ? (KEEP_TEAM_NAMES.has(v) ? v : "Team") : alias.name(v);
    else if (k === "Winner" && typeof v === "string") out[k] = KEEP_TEAM_NAMES.has(v) ? v : "Team";
    else out[k] = scrub(v, alias);
  }
  return out;
}

function assertAnonymized(lines) {
  const walk = (v) => {
    if (Array.isArray(v)) return v.forEach(walk);
    if (!v || typeof v !== "object") return;
    for (const [k, x] of Object.entries(v)) {
      if ((k === "Name" || k === "PlayerName") && !ALLOWED_NAME.test(x)) throw new Error(`Name leaked: ${k}`);
      if (k === "PrimaryId" && !ALLOWED_ID.test(x)) throw new Error("PrimaryId leaked");
      walk(x);
    }
  };
  lines.forEach(walk);
}

function main() {
  const { input, output, me } = parseArgs(process.argv.slice(2));
  const lines = fs
    .readFileSync(input, "utf8")
    .trim()
    .split("\n")
    .map((l) => JSON.parse(l))
    .filter((l) => !l.marker);
  const alias = createAliaser(lines, me ?? detectMe(lines));
  const scrubbed = lines.map((l) => ({ t: l.t, event: l.event, data: scrub(l.data, alias) }));
  assertAnonymized(scrubbed);
  const body = scrubbed.map((l) => JSON.stringify(l)).join("\n") + "\n";
  fs.writeFileSync(output, zlib.gzipSync(body));
  console.log(`Wrote ${scrubbed.length} events to ${output}`);
}

main();
