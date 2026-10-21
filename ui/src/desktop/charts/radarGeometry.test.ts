import { describe, expect, it } from "vitest";
import { at, frame, labelLines, radius, sliceAt } from "./radarGeometry";

describe("radius", () => {
  it("puts the losers' ring at 0.15 + 0.85/4 and the winners' ring at 0.15 + 0.85 * 3/4, clamped to the plot", () => {
    expect(radius(0)).toBeCloseTo(0.3625);
    expect(radius(100)).toBeCloseTo(0.7875);
    expect(radius(-500)).toBeCloseTo(0.15);
    expect(radius(500)).toBe(1);
  });
});

describe("frame", () => {
  it("grows the plot with the width, leaving room for labels, within bounds", () => {
    expect(frame(460).r).toBe(88);
    expect(frame(900).r).toBe(160);
    expect(frame(200).r).toBe(70);
    const f = frame(460);
    expect([f.cx, f.cy, f.h]).toEqual([230, 152, 304]);
  });
});

describe("sliceAt", () => {
  it("picks the axis nearest the pointer's direction", () => {
    expect(sliceAt(0, -50, 5, 100)).toBe(0); // straight up
    expect(sliceAt(60, -20, 5, 100)).toBe(1); // up and right
    expect(sliceAt(-60, -20, 5, 100)).toBe(4);
  });

  it("picks nothing past the reach", () => {
    expect(sliceAt(0, -200, 5, 100)).toBeNull();
  });
});

describe("at", () => {
  it("puts the top axis straight up, with extra pixels outward", () => {
    const f = frame(460);
    expect(at(f, 0, 4, 1)).toEqual([f.cx, f.cy - f.r]);
    expect(at(f, 0, 4, 1, 10)).toEqual([f.cx, f.cy - f.r - 10]);
  });
});

describe("labelLines", () => {
  it("keeps short names on one line and splits long ones near the middle", () => {
    expect(labelLines("Big pads")).toEqual(["Big pads"]);
    expect(labelLines("Touches under 20 boost")).toEqual(["Touches under", "20 boost"]);
    expect(labelLines("In front of the ball")).toEqual(["In front of", "the ball"]);
  });
});
