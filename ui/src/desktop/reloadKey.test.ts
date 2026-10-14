import { describe, expect, it } from "vitest";
import { fixtures } from "@shared/fixtures";
import { reloadKey } from "./reloadKey";

describe("reloadKey", () => {
  it("stays the same for the same state", () => {
    expect(reloadKey(fixtures.idle)).toBe(reloadKey({ ...fixtures.idle }));
  });

  it("changes after a restore (a new engine), not every 1000 snapshots", () => {
    const later = { ...fixtures.idle, version: fixtures.idle.version + 1000 };
    expect(reloadKey(later)).toBe(reloadKey(fixtures.idle));
    const restored = { ...fixtures.idle, generation: fixtures.idle.generation + 1 };
    expect(reloadKey(restored)).not.toBe(reloadKey(fixtures.idle));
  });

  it("changes when a session ends, an activity changes or a review is saved", () => {
    expect(reloadKey(fixtures.sessionEnd)).not.toBe(reloadKey(fixtures.idle));
    expect(reloadKey(fixtures.trainingBelow)).not.toBe(reloadKey(fixtures.menus));
  });
});
