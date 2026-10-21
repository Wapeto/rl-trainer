import assert from "node:assert/strict";
import { test } from "node:test";
import worker, { discordForm, threadName, validateReport } from "./worker.js";

const PNG = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0, 0]);
const headers = (version = "0.2.0") => new Headers(version ? { "X-RL-Trainer-Version": version } : {});

function form(fields = {}, files = {}) {
  const f = new FormData();
  for (const [k, v] of Object.entries({ kind: "bug", text: "it broke", contact: "Sam", ...fields })) f.set(k, v);
  for (const [k, v] of Object.entries(files)) f.set(k, new Blob([v]), k);
  return f;
}

const error = async (f, h = headers()) => {
  const result = await validateReport(f, h);
  assert.equal(result.ok, false);
  return result.error;
};

test("a good report passes with its screenshot and log", async () => {
  const result = await validateReport(form({}, { screenshot: PNG, log: "line" }), headers());
  assert.equal(result.ok, true);
  assert.equal(result.report.screenshot.name, "screenshot.png");
  assert.equal(result.report.log.name, "log.txt");
  const out = discordForm(result.report);
  assert.ok(out.get("files[0]") && out.get("files[1]"));
  assert.equal(JSON.parse(out.get("payload_json")).embeds[0].title, "Bug report");
});

test("each report opens a forum post titled by kind and first line, at most 100 chars", () => {
  const name = (kind, text) => threadName({ kind, text });
  assert.equal(name("idea", "Dark mode\nplease"), "Idea: Dark mode");
  const long = name("bug", "x".repeat(300));
  assert.equal(long.length, 100);
  assert.ok(long.startsWith("Bug: x") && long.endsWith("…"));
});

test("a bad kind is refused", async () => {
  assert.match(await error(form({ kind: "spam" })), /kind/);
});

test("empty and too long text are refused", async () => {
  assert.match(await error(form({ text: "   " })), /text/);
  assert.match(await error(form({ text: "x".repeat(2001) })), /text/);
});

test("a screenshot that is not an image is refused", async () => {
  assert.match(await error(form({}, { screenshot: "GIF89a" })), /PNG or JPEG/);
});

test("an oversize screenshot or log is refused", async () => {
  const big = new Uint8Array(8 * 1024 * 1024 + 1);
  big.set(PNG);
  assert.match(await error(form({}, { screenshot: big })), /8 MB/);
  assert.match(await error(form({}, { log: new Uint8Array(300 * 1024 + 1) })), /300 KB/);
});

test("the version header is required", async () => {
  assert.match(await error(form(), headers(null)), /version/);
});

const PING_ID = "0123456789abcdef0123456789abcdef";

function ping(body, headersIn = headers()) {
  return new Request("http://r/ping", { method: "POST", body: JSON.stringify(body), headers: headersIn });
}

function pingEnv() {
  const puts = [];
  const env = {
    LIMITER: { limit: async () => ({ success: true }) },
    INSTALLS: { put: async (key, value, options) => puts.push({ key, value, options }) },
  };
  return { env, puts };
}

test("a ping stores the install for 30 days", async () => {
  const { env, puts } = pingEnv();
  const res = await worker.fetch(ping({ id: PING_ID, version: "0.3.0" }), env);
  assert.equal(res.status, 204);
  assert.deepEqual(puts, [{ key: PING_ID, value: "0.3.0", options: { expirationTtl: 2592000 } }]);
});

test("a bad ping is refused and stores nothing", async () => {
  const { env, puts } = pingEnv();
  const status = async (req) => (await worker.fetch(req, env)).status;
  assert.equal(await status(ping({ id: "XYZ", version: "0.3.0" })), 400);
  assert.equal(await status(ping({ id: PING_ID.toUpperCase(), version: "0.3.0" })), 400);
  assert.equal(await status(ping({ id: PING_ID, version: "v".repeat(21) })), 400);
  assert.equal(await status(ping({ id: PING_ID, version: "" })), 400);
  assert.equal(await status(ping({ id: PING_ID, version: "0.3.0" }, headers(null))), 400);
  const notJson = new Request("http://r/ping", { method: "POST", body: "nope", headers: headers() });
  assert.equal(await status(notJson), 400);
  assert.equal(puts.length, 0);
});

test("a ping shares the rate limit", async () => {
  const env = { LIMITER: { limit: async () => ({ success: false }) }, INSTALLS: { put: async () => {} } };
  assert.equal((await worker.fetch(ping({ id: PING_ID, version: "0.3.0" }), env)).status, 429);
});

test("only POST /report is served, and the rate limit answers 429", async () => {
  const env = { LIMITER: { limit: async () => ({ success: false }) }, DISCORD_WEBHOOK_URL: "http://x" };
  assert.equal((await worker.fetch(new Request("http://r/other", { method: "POST" }), env)).status, 404);
  const limited = await worker.fetch(
    new Request("http://r/report", { method: "POST", body: form(), headers: headers() }),
    env,
  );
  assert.equal(limited.status, 429);
});
