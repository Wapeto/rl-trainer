import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { copyFileSync, existsSync, mkdtempSync, rmSync, utimesSync } from "node:fs";
import { tmpdir } from "node:os";
import { DatabaseSync } from "node:sqlite";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";

const repo = fileURLToPath(new URL("../..", import.meta.url));
const recording = join(repo, "crates/rl-core/tests/fixtures/training-ranked-2026-10-07.jsonl.gz");
const binary = join(repo, "target/debug", process.platform === "win32" ? "rl-replay.exe" : "rl-replay");
/** An online replay. It holds players' account ids, so it is not committed: without it the test skips. */
const FIXTURE = join(repo, "crates/rl-app/tests/fixtures/match-2026-10.replay");

test.use({ viewport: { width: 1280, height: 800 }, isMobile: false, hasTouch: false });

let server: ChildProcess | null = null;
let dataDir = "";
let replayDir = "";
let url = "";

/** Records the recording, gives its first ranked match the fixture's id and the fixture's player
 * as the profile, then serves the desktop page on loopback with a replay folder of its own. */
async function startServer(): Promise<string> {
  const build = spawnSync("cargo", ["build", "-q", "-p", "rl-cli"], { cwd: repo, stdio: "inherit" });
  if (build.status !== 0) throw new Error("cargo build -p rl-cli failed");
  dataDir = mkdtempSync(join(tmpdir(), "rl-replays-e2e-db-"));
  replayDir = mkdtempSync(join(tmpdir(), "rl-replays-e2e-"));
  const db = join(dataDir, "e2e.db");
  const replay = spawnSync(binary, ["--db", db, "replay", recording], { cwd: repo, stdio: "ignore" });
  if (replay.status !== 0) throw new Error("rl-replay replay failed");
  const header = spawnSync(binary, ["--db", db, "replay-header", FIXTURE], { cwd: repo, encoding: "utf8" });
  if (header.status !== 0) throw new Error(`rl-replay replay-header failed: ${header.stderr}`);
  const { guid, humans } = JSON.parse(header.stdout) as { guid: string; humans: string[] };
  const edit = new DatabaseSync(db);
  edit.exec(`UPDATE match_record SET match_guid = '${guid}'
               WHERE id = (SELECT MIN(id) FROM match_record WHERE ranked = 1);
             UPDATE profile SET primary_id = '${humans[0]}' WHERE is_active = 1;`);
  edit.close();
  const args = ["--db", db, "serve", "--host", "127.0.0.1", "--port", "0", "--desktop-port", "0", "--replay-dir", replayDir];
  const child = spawn(binary, args, { cwd: repo });
  server = child;
  return await new Promise((resolve, reject) => {
    let out = "";
    const timer = setTimeout(() => reject(new Error(`no desktop URL in time:\n${out}`)), 60_000);
    child.stdout!.on("data", (chunk: Buffer) => {
      out += chunk.toString();
      const match = out.match(/Desktop page: (\S+)/);
      if (match) {
        clearTimeout(timer);
        resolve(match[1]);
      }
    });
    child.on("exit", (code) => reject(new Error(`rl-replay exited with ${code}:\n${out}`)));
  });
}

test.beforeAll(async () => {
  if (existsSync(FIXTURE)) url = await startServer();
});

test.afterAll(async () => {
  // Windows keeps the database locked until the process has really exited.
  if (server && server.exitCode === null) {
    const exited = new Promise((resolve) => server!.once("exit", resolve));
    server.kill();
    await exited;
  }
  for (const dir of [dataDir, replayDir]) {
    if (dir) rmSync(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
  }
});

test("a saved replay shows on its match in the session page", async ({ page }) => {
  test.skip(!existsSync(FIXTURE), "no replay fixture (it is not committed)");
  const target = join(replayDir, "match.replay");
  copyFileSync(FIXTURE, target);
  // Old enough to count as fully written at the first look.
  const past = new Date(Date.now() - 10_000);
  utimesSync(target, past, past);
  await page.goto(url);
  await page.getByRole("button", { name: "Show matches" }).first().click();
  await expect(page.getByRole("button", { name: "Replay ▾" })).toBeVisible({ timeout: 30_000 });
  await page.getByRole("button", { name: "Replay ▾" }).click();
  await expect(page.getByText("Low-boost touches")).toBeVisible();
});
