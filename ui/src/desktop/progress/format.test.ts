import { describe, expect, it } from "vitest";
import { changeTone, pointsChange, signed, winPct } from "./format";

describe("progress format", () => {
  it("signs changes with a real minus and shows rate changes in points", () => {
    expect(signed(32)).toBe("+32");
    expect(signed(-9)).toBe("−9");
    expect(signed(0)).toBe("0");
    expect(signed(null)).toBe("—");
    expect(pointsChange(0.07)).toBe("+7 pts");
    expect(pointsChange(null)).toBe("—");
  });

  it("colours a change and leaves out unknown results", () => {
    expect([changeTone(3), changeTone(-1), changeTone(0), changeTone(null)]).toEqual(["d-g", "d-r", "d-m", "d-m"]);
    expect(winPct(3, 1)).toBe(75);
    expect(winPct(0, 0)).toBeNull();
  });
});
