import { describe, expect, it } from "vitest";
import type { SessionTime } from "@shared/generated/types";
import { weekBuckets, weekStart } from "./weeks";

const at = (y: number, m: number, d: number, h = 20) => new Date(y, m - 1, d, h).getTime();
const session = (ms: number, training: number, ranked: number): SessionTime => ({
  session_id: ms,
  at: ms,
  training_ms: training,
  freeplay_ms: 0,
  ranked_ms: ranked,
  other_ms: 0,
});

describe("weeks", () => {
  it("starts a week on Monday at midnight, local time", () => {
    expect(weekStart(at(2026, 10, 11))).toBe(new Date(2026, 9, 5).getTime()); // Sunday → Monday 5 Oct
    expect(weekStart(at(2026, 10, 12))).toBe(new Date(2026, 9, 12).getTime());
  });

  it("adds sessions into their week and keeps empty weeks", () => {
    const weeks = weekBuckets(
      [session(at(2026, 9, 29), 60_000, 0), session(at(2026, 10, 1), 30_000, 90_000), session(at(2026, 10, 14), 10_000, 0)],
      at(2026, 9, 28),
      at(2026, 10, 15),
    );
    expect(weeks.map((w) => [w.training, w.ranked])).toEqual([
      [90_000, 90_000],
      [0, 0],
      [10_000, 0],
    ]);
  });
});
