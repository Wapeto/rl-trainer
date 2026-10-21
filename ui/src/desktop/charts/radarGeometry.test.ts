import { describe, expect, it } from "vitest";
import { CX, CY, H, R, W, at, dockCard, radius, sliceAt } from "./radarGeometry";

describe("radius", () => {
  it("puts the losers' ring at 0.15 + 0.85/4 and the winners' ring at 0.15 + 0.85 * 3/4, clamped to the plot", () => {
    expect(radius(0)).toBeCloseTo(0.3625);
    expect(radius(100)).toBeCloseTo(0.7875);
    expect(radius(-500)).toBeCloseTo(0.15);
    expect(radius(500)).toBe(1);
  });
});

describe("sliceAt", () => {
  it("picks the axis nearest the pointer's direction", () => {
    expect(sliceAt(0, -50, 5)).toBe(0); // straight up
    expect(sliceAt(60, -20, 5)).toBe(1); // up and right
    expect(sliceAt(-60, -20, 5)).toBe(4);
  });

  it("picks nothing past the reach", () => {
    expect(sliceAt(0, -R * 2, 5)).toBeNull();
  });
});

describe("dockCard", () => {
  const clears = (x: number, y: number, w: number, h: number) =>
    Math.hypot(Math.max(x, Math.min(CX, x + w)) - CX, Math.max(y, Math.min(CY, y + h)) - CY) > R;

  it.each([3, 5, 6, 8])("keeps the card off the plot and inside the chart for every axis of %i", (n) => {
    for (let i = 0; i < n; i++) {
      const box = dockCard(i, n, 210, 120);
      expect(box, `axis ${i}`).not.toBeNull();
      const { x, y } = box!;
      expect(clears(x, y, 210, 120), `axis ${i}`).toBe(true);
      expect(x >= 0 && y >= 0 && x + 210 <= W && y + 120 <= H, `axis ${i}`).toBe(true);
    }
  });

  it("gives up when the card is too big for the chart, so it goes under it", () => {
    expect(dockCard(1, 5, 480, 300)).toBeNull();
  });

  it("puts the top axis straight up", () => {
    expect(at(0, 4, 1)).toEqual([CX, CY - R]);
  });
});
