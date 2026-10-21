import { describe, expect, it } from "vitest";
import type { BenchStat, Benchmark, PlayerReplayStats } from "@shared/generated/types";
import { direction, groupAxes, nothingSeparated, ordinal, overviewAxes, progressAxes, progressGroupAxes } from "./benchRadar";
import { STAT_GROUPS } from "./replayRows";

const group = (name: string) => STAT_GROUPS.find((g) => g.name === name)!;
const st = (gap: number, me: number | null = 50, winners_pct = 60): BenchStat => ({ gap, separates: Math.abs(gap) >= 0.3, me, winners_pct });
const bench = (stats: Record<string, BenchStat>): Benchmark => ({
  matches: 20,
  winners: { tracked_seconds: 300, boost_collected: 3000 } as PlayerReplayStats,
  losers: { tracked_seconds: 300, boost_collected: 2500 } as PlayerReplayStats,
  stats,
});

describe("groupAxes", () => {
  it("keeps the 6 largest gaps, in the group's order", () => {
    const b = bench({
      boost_collected: st(0.9), boost_used: st(0.3), boost_stolen: st(-0.8), big_pads: st(0.35),
      small_pads: st(0.7), pct_zero_boost: st(-1), pct_full_boost: st(0.4), boost_used_supersonic: st(-0.5),
    });
    expect(groupAxes(group("Boost"), b, true).map((a) => a.key)).toEqual([
      "boost_collected", "boost_stolen", "small_pads", "pct_zero_boost", "pct_full_boost", "boost_used_supersonic",
    ]);
  });

  it("skips stats that do not separate, and team-only groups in 1v1", () => {
    const b = bench({ boost_collected: st(0.2), big_pads: st(0.5), secs_first_man: st(0.9) });
    expect(groupAxes(group("Boost"), b, true).map((a) => a.key)).toEqual(["big_pads"]);
    expect(groupAxes(group("Rotation"), b, false)).toEqual([]);
  });

  it("explains each axis with the winners' and losers' values", () => {
    const row = group("Boost").rows.find((r) => r.key === "boost_collected")!;
    const [axis] = groupAxes(group("Boost"), bench({ boost_collected: st(0.9, 40, 62) }), true);
    expect(axis).toMatchObject({ label: row.label, me: 40, winners: 62 });
    expect(axis.tip).toBe(`${row.label}: you 40th percentile, winners 62nd; winners ${row.format(3000)}, losers ${row.format(2500)}, more is better`);
  });

  it("gives the card the real values: the winners' mean, mine on a match page, and the unit", () => {
    const row = group("Boost").rows.find((r) => r.key === "boost_collected")!;
    const b = bench({ boost_collected: st(0.9, 40, 62) });
    const mine = { ...b.winners!, boost_collected: 2000 };
    const [axis] = groupAxes(group("Boost"), b, true, mine);
    expect(axis).toMatchObject({ winnersValue: row.format(3000), you: row.format(2000), unit: "per 5 min" });
    expect(groupAxes(group("Boost"), b, true)[0].you).toBeUndefined();
  });
});

describe("overviewAxes", () => {
  const b = bench({
    boost_collected: st(0.9, 40, 60), big_pads: st(0.5, null, 70), pct_zero_boost: st(-1, 20, 80),
    touches: st(0.5), hard_hits: st(0.5),
    secs_first_man: st(0.5), secs_second_man: st(0.5), pct_most_back: st(0.5),
  });

  it("averages each group with a radar, ignoring my missing values", () => {
    expect(overviewAxes(STAT_GROUPS, b, true)).toEqual([
      { key: "Rotation", label: "Rotation", me: 50, winners: 60, unit: "average rank" },
      { key: "Boost", label: "Boost", me: 30, winners: 70, unit: "average rank" },
    ]);
  });

  it("team_only_groups_are_skipped_in_1v1", () => {
    expect(overviewAxes(STAT_GROUPS, b, false).map((a) => a.key)).toEqual(["Boost"]);
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

describe("ordinal", () => {
  it("rounds and picks the suffix", () => {
    expect([1, 2, 3, 4, 11, 12, 13, 21, 22, 61.4, 99.6].map(ordinal)).toEqual(
      ["1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st", "22nd", "61st", "100th"],
    );
  });
});

describe("progressAxes", () => {
  const b = bench({
    boost_collected: st(0.9, 40, 60), big_pads: st(0.5, 20, 80),
    touches: st(0.1), hard_hits: st(-0.2),
    secs_first_man: st(0.5, null, 70),
  });
  const keys = (team: boolean) => progressAxes(STAT_GROUPS, b, team).map((a) => a.key);

  it("has one axis per measured group: separating stats averaged, the rest dimmed at 50", () => {
    const axes = progressAxes(STAT_GROUPS, b, true);
    expect(axes.map((a) => a.key)).toEqual(["Rotation", "Boost", "Touches"]);
    expect(axes[1]).toEqual({ key: "Boost", label: "Boost", me: 30, winners: 70, tip: "Boost: you 30th percentile, winners 70th", unit: "average rank" });
    expect(axes[2]).toEqual({ key: "Touches", label: "Touches", me: 50, winners: 50, dim: true, tip: nothingSeparated("Touches", 20) });
  });

  it("shows a missing value of mine as a dash, and skips team-only groups in 1v1", () => {
    expect(progressAxes(STAT_GROUPS, b, true)[0]).toMatchObject({ me: null, tip: "Rotation: you —, winners 70th" });
    expect(keys(false)).toEqual(["Boost", "Touches"]);
  });
});

describe("progressGroupAxes", () => {
  it("draws every measured stat of the group, dimming those that do not separate", () => {
    const b = bench({ boost_collected: st(0.9, 40, 62), big_pads: st(0.1), boost_stolen: st(-0.2) });
    const axes = progressGroupAxes(group("Boost"), b, true);
    expect(Object.fromEntries(axes.map((a) => [a.key, a.dim ?? false]))).toEqual({ boost_collected: false, big_pads: true, boost_stolen: true });
    const row = group("Boost").rows.find((r) => r.key === "big_pads")!;
    expect(axes.find((a) => a.key === "big_pads")).toMatchObject({
      me: 50,
      winners: 50,
      tip: `${row.label}: winners —, losers —, no clear difference between winners and losers`,
    });
  });

  it("skips team-only groups in 1v1", () => {
    expect(progressGroupAxes(group("Rotation"), bench({ secs_first_man: st(0.5) }), false)).toEqual([]);
  });
});
