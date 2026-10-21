import { describe, expect, it } from "vitest";
import type { BenchStat, Benchmark, PlayerReplayStats } from "@shared/generated/types";
import { direction, groupAxes, nothingSeparated, overviewAxes, progressAxes } from "./benchRadar";
import { STAT_GROUPS } from "./replayRows";

const group = (name: string) => STAT_GROUPS.find((g) => g.name === name)!;
const st = (gap: number): BenchStat => ({ gap, separates: Math.abs(gap) >= 0.3, me: 50, winners_pct: 60 });
const stats = (s: Partial<PlayerReplayStats>) => ({ tracked_seconds: 300, ...s }) as PlayerReplayStats;
/** Winners collect 3000 boost, losers 2500, I 2750: halfway. Other stats default to the same shape. */
const bench = (b: Record<string, BenchStat>, over: Partial<Benchmark> = {}): Benchmark => ({
  matches: 20,
  winners: stats({ boost_collected: 3000, big_pads: 12, pct_zero_boost: 8, touches: 40, secs_first_man: 70 }),
  losers: stats({ boost_collected: 2500, big_pads: 8, pct_zero_boost: 14, touches: 30, secs_first_man: 50 }),
  mine: stats({ boost_collected: 2750, big_pads: 8, pct_zero_boost: 14, touches: 35, secs_first_man: 60 }),
  stats: b,
  ...over,
});

describe("groupAxes", () => {
  it("keeps the 6 that matter most, in the group's order", () => {
    const b = bench({
      boost_collected: st(0.9), boost_used: st(0.3), boost_stolen: st(-0.8), big_pads: st(0.35),
      small_pads: st(0.7), pct_zero_boost: st(-1), pct_full_boost: st(0.4), boost_used_supersonic: st(-0.5),
    });
    const w = b.winners!;
    const full = { ...b, winners: { ...w, boost_used: 1, boost_stolen: 1, small_pads: 9, pct_full_boost: 3, boost_used_supersonic: 1 } };
    full.losers = { ...b.losers!, boost_used: 2, boost_stolen: 3, small_pads: 5, pct_full_boost: 1, boost_used_supersonic: 4 };
    full.mine = { ...b.mine!, boost_used: 2, boost_stolen: 3, small_pads: 5, pct_full_boost: 1, boost_used_supersonic: 4 };
    expect(groupAxes(group("Boost"), full, true).map((a) => a.key)).toEqual([
      "boost_collected", "boost_stolen", "small_pads", "pct_zero_boost", "pct_full_boost", "boost_used_supersonic",
    ]);
  });

  it("skips stats that do not separate, and team-only groups in 1v1", () => {
    const b = bench({ boost_collected: st(0.2), big_pads: st(0.5), secs_first_man: st(0.9) });
    expect(groupAxes(group("Boost"), b, true).map((a) => a.key)).toEqual(["big_pads"]);
    expect(groupAxes(group("Rotation"), b, false)).toEqual([]);
  });

  it("puts me between the losers' level (0) and the winners' (100) in real values", () => {
    const row = group("Boost").rows.find((r) => r.key === "boost_collected")!;
    const [axis] = groupAxes(group("Boost"), bench({ boost_collected: st(0.9) }), true);
    expect(axis).toEqual({
      key: "boost_collected", label: row.label, t: 50, importance: 0.9,
      you: row.format(2750), winnersValue: row.format(3000), losersValue: row.format(2500), unit: "per 5 min", lessBetter: false,
    });
  });

  it("flips the scale when less is better, and goes past 100 or below 0", () => {
    const b = bench({ pct_zero_boost: st(-1), big_pads: st(0.5) });
    const [pads, zero] = groupAxes(group("Boost"), { ...b, mine: stats({ pct_zero_boost: 5, big_pads: 6 }) }, true);
    expect(pads).toMatchObject({ key: "big_pads", lessBetter: false });
    expect(pads.t).toBeCloseTo(-50); // 6 pads against 8 (0) and 12 (100)
    expect(zero).toMatchObject({ key: "pct_zero_boost", lessBetter: true });
    expect(zero.t).toBeCloseTo(150); // 5% against 14% (0) and 8% (100)
  });

  it("has no position without a value of mine, and no axis when winners and losers are equal", () => {
    const b = bench({ boost_collected: st(0.9), big_pads: st(0.5) });
    expect(groupAxes(group("Boost"), { ...b, mine: null }, true).map((a) => a.t)).toEqual([null, null]);
    const same = { ...b, losers: { ...b.losers!, big_pads: 12 } };
    expect(groupAxes(group("Boost"), same, true).map((a) => a.key)).toEqual(["boost_collected"]);
  });
});

describe("overviewAxes", () => {
  const b = bench({
    boost_collected: st(0.9), big_pads: st(0.5), pct_zero_boost: st(-1),
    secs_first_man: st(0.5), secs_second_man: st(0.5), pct_most_back: st(0.5),
  });

  it("averages each group with a separating stat: position and importance", () => {
    expect(overviewAxes(STAT_GROUPS, b, true)).toEqual([
      { key: "Rotation", label: "Rotation", t: 50, importance: 0.5, unit: "from 1 stat" },
      { key: "Boost", label: "Boost", t: expect.closeTo((50 + 0 + 0) / 3, 5), importance: expect.closeTo((0.9 + 0.5 + 1) / 3, 5), unit: "average of 3 stats" },
    ]);
  });

  it("skips team-only groups in 1v1", () => {
    expect(overviewAxes(STAT_GROUPS, b, false).map((a) => a.key)).toEqual(["Boost"]);
    expect(overviewAxes(STAT_GROUPS, bench({ boost_collected: st(0.1) }), true)).toEqual([]);
  });
});

describe("direction", () => {
  it("names the better way only for separating stats", () => {
    expect(direction(st(0.5))).toBe("more is better");
    expect(direction(st(-0.5))).toBe("less is better");
    expect(direction(st(0.1))).toBeNull();
    expect(direction(undefined)).toBeNull();
  });
});

describe("progressAxes", () => {
  const b = bench({ boost_collected: st(0.9), big_pads: st(0.5), touches: st(0.1), hard_hits: st(-0.2), secs_first_man: st(0.5) });
  const keys = (team: boolean) => progressAxes(STAT_GROUPS, b, team).map((a) => a.key);

  it("has one axis per measured group: separating stats averaged, the rest dimmed", () => {
    const axes = progressAxes(STAT_GROUPS, b, true);
    expect(axes.map((a) => a.key)).toEqual(["Rotation", "Boost", "Touches"]);
    expect(axes[1]).toMatchObject({ key: "Boost", t: 25, unit: "average of 2 stats" });
    expect(axes[2]).toEqual({ key: "Touches", label: "Touches", t: null, importance: 0, dim: true, tip: nothingSeparated("Touches", 20) });
  });

  it("skips team-only groups in 1v1", () => {
    expect(keys(false)).toEqual(["Boost", "Touches"]);
  });
});
