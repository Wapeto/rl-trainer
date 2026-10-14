import { afterEach, describe, expect, it, vi } from "vitest";
import { fixtures } from "@shared/fixtures";
import type { AppSnapshot } from "@shared/generated/types";
import type { DesktopApi } from "./api";
import { LiveFeed, followTauri, poll } from "./live";

const at = (version: number): AppSnapshot => ({ ...fixtures.idle, version });

describe("LiveFeed", () => {
  it("tells subscribers and never goes back to an older snapshot", () => {
    const feed = new LiveFeed();
    const seen = vi.fn();
    feed.subscribe(seen);
    feed.set(at(5));
    feed.set(at(3));
    expect(feed.getState()?.version).toBe(5);
    expect(seen).toHaveBeenCalledTimes(1);
  });
});

describe("feeds", () => {
  afterEach(() => vi.useRealTimers());

  it("polls the snapshot in a browser", async () => {
    vi.useFakeTimers();
    const snapshot = vi.fn().mockResolvedValueOnce(at(1)).mockResolvedValue(at(2));
    const feed = new LiveFeed();
    const stop = poll(feed, { snapshot } as unknown as DesktopApi, 1000);
    await vi.advanceTimersByTimeAsync(1000);
    expect(feed.getState()?.version).toBe(2);
    stop();
    await vi.advanceTimersByTimeAsync(5000);
    expect(snapshot).toHaveBeenCalledTimes(2);
  });

  it("follows the live event in the app", async () => {
    let handler: ((e: { payload: unknown }) => void) | null = null;
    const listen = vi.fn(async (_name: string, h: (e: { payload: unknown }) => void) => {
      handler = h;
      return () => {};
    });
    const feed = new LiveFeed();
    const api = { snapshot: vi.fn().mockResolvedValue(at(10)) } as unknown as DesktopApi;
    await followTauri(feed, { core: { invoke: vi.fn() }, event: { listen } }, api);
    expect(listen).toHaveBeenCalledWith("live", expect.any(Function));
    handler!({ payload: at(11) });
    expect(feed.getState()?.version).toBe(11);
  });
});
