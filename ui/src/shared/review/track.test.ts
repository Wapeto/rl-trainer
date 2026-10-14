import { describe, expect, it } from "vitest";
import type { ReviewItem } from "@shared/generated/types";
import { trackScale } from "./track";

const item = (over: Partial<ReviewItem>): ReviewItem => ({
  key: "behind_ball",
  label: "Behind the ball",
  unit: "percent",
  value: 90,
  usual: 50,
  better: true,
  spread: 10,
  covered: 1,
  of: 1,
  ...over,
});

describe("trackScale", () => {
  it("places the band, the usual and a tick far above it", () => {
    const t = trackScale(item({}));
    expect(t.bandFrom).toBeCloseTo(21.3, 0);
    expect(t.bandTo).toBeCloseTo(50, 0);
    expect(t.usual).toBeCloseTo(35.6, 0);
    expect(t.tick).toBeCloseTo(93.1, 0);
  });

  it("keeps percentages between 0 and 100", () => {
    const t = trackScale(item({ value: 2, usual: 5, spread: 4 }));
    expect(t.tick).toBeCloseTo(14, 0);
    expect(t.bandFrom).toBeCloseTo(7, 0);
    expect(t.bandTo).toBeCloseTo(63, 0);
  });

  it("keeps everything on the track whatever the numbers", () => {
    for (const over of [{ value: 1000 }, { value: -50, unit: "per_game" as const }, { spread: 0.0001, value: 50 }]) {
      const t = trackScale(item(over));
      for (const v of [t.bandFrom, t.bandTo, t.usual, t.tick]) {
        expect(v).toBeGreaterThanOrEqual(0);
        expect(v).toBeLessThanOrEqual(100);
      }
      expect(t.bandFrom).toBeLessThanOrEqual(t.bandTo);
    }
  });
});
