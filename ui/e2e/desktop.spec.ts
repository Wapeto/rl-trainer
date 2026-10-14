import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { DatabaseSync } from "node:sqlite";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";

const repo = fileURLToPath(new URL("../..", import.meta.url));
const recording = join(repo, "crates/rl-core/tests/fixtures/training-ranked-2026-10-07.jsonl.gz");
const binary = join(repo, "target/debug", process.platform === "win32" ? "rl-replay.exe" : "rl-replay");

test.use({ viewport: { width: 1280, height: 800 }, isMobile: false, hasTouch: false });

let server: ChildProcess | null = null;
let dataDir = "";

/** Records the recording into a fresh database, then serves the desktop page on loopback. */
async function startServer(): Promise<string> {
  const build = spawnSync("cargo", ["build", "-q", "-p", "rl-cli"], { cwd: repo, stdio: "inherit" });
  if (build.status !== 0) throw new Error("cargo build -p rl-cli failed");
  dataDir = mkdtempSync(join(tmpdir(), "rl-desktop-e2e-"));
  const db = join(dataDir, "e2e.db");
  // The replay prints every fact; it is not needed here.
  const replay = spawnSync(binary, ["--db", db, "replay", recording], { cwd: repo, stdio: "ignore" });
  if (replay.status !== 0) throw new Error("rl-replay replay failed");
  // The recording's breaks are all under a minute; move its last activity 6 minutes later so the
  // session has a break long enough to offer "Split here".
  const edit = new DatabaseSync(db);
  edit.exec(`UPDATE block SET started_at = started_at + 360000, ended_at = ended_at + 360000
             WHERE id = (SELECT MAX(id) FROM block);
             UPDATE session SET ended_at = ended_at + 360000;`);
  edit.close();
  const args = ["--db", db, "serve", "--host", "127.0.0.1", "--port", "0", "--desktop-port", "0"];
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

let url = "";

// One server for the file: each start leaves its process and data folder to the shared afterAll.
test.beforeAll(async () => {
  url = await startServer();
});

test.afterAll(async () => {
  // Windows keeps the database locked until the process has really exited.
  if (server && server.exitCode === null) {
    const exited = new Promise((resolve) => server!.once("exit", resolve));
    server.kill();
    await exited;
  }
  if (dataDir) rmSync(dataDir, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
});

test("the desktop window edits a recorded session and adds a pack", async ({ page }) => {
  await page.goto(url);
  // Session lanes only: the "replays to save" lane above them is a filter, not a session.
  const lanes = page.getByRole("region", { name: "Sessions" }).locator(".d-lane:not(.d-lane--save)");
  await expect(lanes).toHaveCount(1, { timeout: 30_000 });
  await expect(page.getByRole("heading", { level: 1 })).toBeVisible();

  // Set the pack of the training recorded without one.
  await page.getByRole("button", { name: "Set pack" }).first().click();
  const picker = page.getByRole("dialog", { name: "Which pack was this?" });
  await picker.getByLabel("Search packs").fill("aerial shots");
  await picker.getByRole("option", { name: /Aerial Shots - Pass/ }).click();
  await picker.getByRole("button", { name: "Set" }).click();
  await expect(page.locator(".d-ev__t", { hasText: "Aerial Shots - Pass" }).first()).toBeVisible();

  // Edit the review.
  await page.getByRole("button", { name: "Edit review" }).click();
  const review = page.getByRole("dialog", { name: /^Review ·/ });
  const mmr = review.getByLabel(/v\d MMR/);
  if ((await mmr.count()) > 0) await mmr.first().fill("905");
  await review.getByLabel("Note").fill("e2e note");
  await review.getByRole("button", { name: "Save" }).click();
  await expect(page.locator(".d-note", { hasText: "e2e note" })).toBeVisible();

  // Split the session at its first gap, then merge it back.
  await page.getByRole("button", { name: "Split here" }).first().click();
  await page.getByRole("dialog", { name: "Split this session here?" }).getByRole("button", { name: "Split" }).click();
  await expect(lanes).toHaveCount(2);
  await lanes.first().click();
  await page.getByRole("button", { name: "Session actions" }).click();
  await page.getByRole("menuitem", { name: "Merge with previous session" }).click();
  await page.getByRole("dialog", { name: "Merge these two sessions?" }).getByRole("button", { name: "Merge" }).click();
  await expect(lanes).toHaveCount(1);

  // Add a pack with a code typed with spaces.
  await page.getByRole("button", { name: /Packs/ }).click();
  await page.getByRole("button", { name: "Add" }).click();
  const add = page.getByRole("dialog", { name: "Add a pack" });
  await add.getByLabel("Code").fill("1111 2222 3333 4444");
  await add.getByLabel("Name").fill("E2E Pack");
  await add.getByRole("button", { name: "Add pack" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "E2E Pack" })).toBeVisible();
  await expect(page.getByText("1111-2222-3333-4444")).toBeVisible();
});

test("the Progress page shows the recorded ranked matches and opens a session from a chart", async ({ page }) => {
  await page.goto(url);
  await page.getByRole("button", { name: /Progress/ }).click();
  const list = page.getByRole("navigation", { name: "Progress" });
  await list.getByRole("radio", { name: "All" }).click();
  await list.getByRole("button", { name: /^2v2/ }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Ranked 2v2" })).toBeVisible();
  await page.getByRole("tab", { name: "Touches" }).click();
  await page.getByRole("button", { name: "Aerial touches" }).click();
  const focus = page.getByRole("region", { name: "Selected stat" });
  await expect(focus.getByRole("img", { name: "Aerial touches" })).toBeVisible();
  await page.getByRole("img", { name: "Win rate" }).locator("circle").first().click();
  await expect(page.getByRole("region", { name: "Sessions" })).toBeVisible();
  await expect(page.locator(".d-lane--on")).toBeVisible();
  // The first test set this pack on the recording's training.
  await page.getByRole("button", { name: /Progress/ }).click();
  await list.getByRole("button", { name: /Aerial Shots - Pass/ }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Aerial Shots - Pass" })).toBeVisible();
});
