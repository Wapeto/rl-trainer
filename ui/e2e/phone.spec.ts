import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";

const repo = fileURLToPath(new URL("../..", import.meta.url));
const recording = join(repo, "crates/rl-core/tests/fixtures/training-ranked-2026-10-07.jsonl.gz");
const binary = join(repo, "target/debug", process.platform === "win32" ? "rl-replay.exe" : "rl-replay");

const servers: ChildProcess[] = [];
const dataDirs: string[] = [];

function build() {
  const result = spawnSync("cargo", ["build", "-q", "-p", "rl-cli"], { cwd: repo, stdio: "inherit" });
  if (result.status !== 0) throw new Error("cargo build -p rl-cli failed");
}

/** Starts `rl-replay serve` on loopback, replaying the recording at 2x, and returns the pairing URL. */
async function startServer(): Promise<string> {
  build();
  const dataDir = mkdtempSync(join(tmpdir(), "rl-e2e-"));
  dataDirs.push(dataDir);
  const args = ["--db", join(dataDir, "e2e.db"), "serve", "--host", "127.0.0.1", "--port", "0", "--recording", recording, "--speed", "2"];
  const child = spawn(binary, args, { cwd: repo });
  servers.push(child);
  return await pairingUrl(child);
}

/** Records the recording first, then serves it with nothing live: the phone opens on the end of
 * the recorded (unreviewed) session. */
async function startRecordedServer(): Promise<string> {
  build();
  const dataDir = mkdtempSync(join(tmpdir(), "rl-e2e-"));
  dataDirs.push(dataDir);
  const db = join(dataDir, "e2e.db");
  const replay = spawnSync(binary, ["--db", db, "replay", recording], { cwd: repo, stdio: "ignore" });
  if (replay.status !== 0) throw new Error("rl-replay replay failed");
  const child = spawn(binary, ["--db", db, "serve", "--host", "127.0.0.1", "--port", "0"], { cwd: repo });
  servers.push(child);
  return await pairingUrl(child);
}

/** Reads the pairing URL from a starting server's output. */
function pairingUrl(child: ChildProcess): Promise<string> {
  return new Promise((resolve, reject) => {
    let out = "";
    const timer = setTimeout(() => reject(new Error(`no pairing URL in time:\n${out}`)), 60_000);
    child.stdout!.on("data", (chunk: Buffer) => {
      out += chunk.toString();
      const match = out.match(/Open on your phone: (\S+)/);
      if (match) {
        clearTimeout(timer);
        resolve(match[1]);
      }
    });
    child.on("exit", (code) => reject(new Error(`rl-replay exited with ${code}:\n${out}`)));
  });
}

test.afterAll(async () => {
  // Windows keeps the database locked until the process has really exited.
  for (const server of servers) {
    if (server.exitCode === null) {
      const exited = new Promise((resolve) => server.once("exit", resolve));
      server.kill();
      await exited;
    }
  }
  for (const dir of dataDirs) rmSync(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
});

test("the phone follows a recorded training session and match", async ({ page }) => {
  const url = await startServer();
  // The phone opens the page over plain HTTP, where there is no Wake Lock API: hide it here
  // (127.0.0.1 counts as secure) so the video fallback is what gets tested.
  await page.addInitScript(() => {
    Object.defineProperty(Navigator.prototype, "wakeLock", { get: () => undefined });
  });
  await page.goto(url);

  // One tap keeps the screen on: the hidden looping video plays.
  const hint = page.getByText("Tap to keep the screen on");
  await hint.tap();
  await expect(hint).toBeHidden();
  await expect
    .poll(() => page.locator("video.keep-awake-video").evaluate((v: HTMLVideoElement) => !v.paused && v.currentTime > 0))
    .toBe(true);

  // Training: the ring fills from live attempts.
  await expect(page.locator("main")).toHaveAttribute("data-screen", "training", { timeout: 30_000 });
  await expect(page.getByRole("heading", { name: "No pack set" })).toBeVisible();
  await expect(page.locator(".hero__number")).not.toHaveText("—", { timeout: 60_000 });
  await expect(page.locator(".dot--make, .dot--miss").first()).toBeVisible();

  // Set a pack through the picker.
  await page.getByRole("button", { name: "Change pack" }).click();
  const dialog = page.getByRole("dialog", { name: "Choose pack" });
  await dialog.getByLabel("Search packs").fill("aerial shots");
  await dialog.getByRole("button", { name: /Aerial Shots - Pass/ }).click();
  await expect(page.getByRole("heading", { name: "Aerial Shots - Pass" })).toBeVisible();

  // Skip, then undo, the next attempt recorded on the new pack.
  const skip = page.getByRole("button", { name: "Skip last" });
  await expect(skip).toBeEnabled({ timeout: 60_000 });
  await skip.click();
  await expect(page.getByRole("button", { name: "Undo skip" })).toBeVisible();
  await page.getByRole("button", { name: "Undo skip" }).click();
  await expect(page.getByRole("button", { name: "Skip last" })).toBeVisible();

  // The ranked match follows in the recording.
  await expect(page.locator("main")).toHaveAttribute("data-screen", "match", { timeout: 180_000 });
  await expect(page.getByLabel(/^Score \d+ to \d+$/)).toBeVisible();
  await expect(page.locator(".cmp__row")).toHaveCount(5);
});

test("the phone ends a session with the review, then the MMR wheel", async ({ page }) => {
  const url = await startRecordedServer();
  await page.goto(url);
  await expect(page.locator("main")).toHaveAttribute("data-screen", "session_end", { timeout: 30_000 });
  await expect(page.getByRole("heading", { name: "Session review" })).toBeVisible();
  await page.getByRole("button", { name: "Next: MMR & note" }).click();
  // A fresh database has no MMR yet: the wheel starts as a number field, then turns.
  const field = page.getByRole("textbox", { name: /^MMR [0-9]v[0-9]$/ }).first();
  await field.fill("900");
  await field.press("Enter");
  const wheel = page.getByRole("spinbutton", { name: /^MMR [0-9]v[0-9]$/ }).first();
  await expect(wheel).toHaveAttribute("aria-valuenow", "900");
  await wheel.focus();
  await page.keyboard.press("PageUp");
  await expect(wheel).toHaveAttribute("aria-valuenow", "910");
  await page.getByRole("button", { name: "Save" }).click();
  await expect(page.locator("main")).toHaveAttribute("data-screen", "idle", { timeout: 10_000 });
  await expect(page.getByText("910").first()).toBeVisible();
});
