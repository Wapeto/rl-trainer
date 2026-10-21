import { describe, expect, it } from "vitest";
import { CX, CY, H, R, W, at, dockCard, radarScale, ringValues, sliceAt } from "./radarGeometry";

describe("radarScale", () => {
  it("zooms in when every value sits in a narrow band", () => {
    expect(radarScale([52, 66, 61, 68, 57, 60, 72, 58, 38, 71])).toEqual({ lo: 30, hi: 80, zoomed: true });
  });

  it("keeps 0–100 when values spread or come close to the centre", () => {
    expect(radarScale([9, 94, 50])).toEqual({ lo: 0, hi: 100, zoomed: false });
    expect(radarScale([24, 40])).toEqual({ lo: 0, hi: 100, zoomed: false });
    expect(radarScale([])).toEqual({ lo: 0, hi: 100, zoomed: false });
  });

  it("puts rings at round values, the outer edge last", () => {
    expect(ringValues({ lo: 30, hi: 80, zoomed: true })).toEqual([40, 50, 60, 70, 80]);
    expect(ringValues({ lo: 0, hi: 100, zoomed: false })).toEqual([25, 50, 75, 100]);
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
