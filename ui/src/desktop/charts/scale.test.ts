import { describe, expect, it } from "vitest";
import { axisDates, axisValue, domain, monthTicks, scale } from "./scale";

describe("chart scales", () => {
  it("maps a domain onto a range, and a single value to the middle", () => {
    expect(scale(0, 10, 100, 200)(5)).toBe(150);
    expect(scale(4, 4, 0, 100)(4)).toBe(50);
  });

  it("pads the drawn values by 10%, and a flat line by 1", () => {
    expect(domain([10, 20])).toEqual([9, 21]);
    expect(domain([5, 5])).toEqual([4, 6]);
    expect(domain([])).toEqual([0, 1]);
  });

  it("puts a tick on the first day of each month in between", () => {
    const ticks = monthTicks(new Date(2026, 8, 15).getTime(), new Date(2026, 10, 20).getTime());
    expect(ticks).toEqual([new Date(2026, 9, 1).getTime(), new Date(2026, 10, 1).getTime()]);
  });

  it("writes axis values with a decimal under 10", () => {
    expect(axisValue(0.75, "")).toBe("0.8");
    expect(axisValue(64.4, "%")).toBe("64%");
  });

  it("never pads below zero when every value is zero or more", () => {
    expect(domain([0, 2])).toEqual([0, 2.2]);
    expect(domain([0, 0])).toEqual([0, 1]);
    expect(domain([-1, 1])).toEqual([-1.2, 1.2]);
  });

  it("dates the axis by month over long spans and by the first and last day over short ones", () => {
    const long = axisDates(new Date(2026, 6, 20).getTime(), new Date(2026, 9, 9).getTime());
    expect(long.map((t) => t.label)).toEqual(["Aug", "Sept", "Oct"]);
    const short = axisDates(new Date(2026, 9, 1, 20).getTime(), new Date(2026, 9, 9, 21).getTime());
    expect(short.map((t) => [t.label, t.anchor])).toEqual([
      ["1 Oct", "start"],
      ["9 Oct", "end"],
    ]);
    const one = new Date(2026, 9, 9, 21).getTime();
    expect(axisDates(one, one).map((t) => t.label)).toEqual(["9 Oct"]);
  });
});
