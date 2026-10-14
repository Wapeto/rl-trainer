import type { BenchStat, Benchmark, PlayerReplayStats } from "@shared/generated/types";
import type { RadarAxis } from "../charts/Radar";
import { type StatGroup, type StatRow, statValue } from "./replayRows";

/** Most axes on a group radar, and fewest worth drawing (winners' benchmark spec §4.2). */
export const MAX_AXES = 6;
export const MIN_AXES = 3;

/** Which way the winners lean on a stat, when they clearly differ from the losers. */
export function direction(s: BenchStat | undefined): string | null {
  return s?.separates ? (s.gap > 0 ? "more is better" : "less is better") : null;
}

/** 1st, 2nd, 3rd, 4th… of a rounded percentile. */
export function ordinal(n: number): string {
  const r = Math.round(n);
  const teen = r % 100 >= 11 && r % 100 <= 13;
  const suffix = teen ? "th" : (({ 1: "st", 2: "nd", 3: "rd" } as Record<number, string>)[r % 10] ?? "th");
  return `${r}${suffix}`;
}

/** Why a group has no radar, or a dimmed Progress axis. */
export function nothingSeparated(group: string, matches: number): string {
  return `Nothing in ${group} separated winners from losers in your last ${matches} matches.`;
}

const percentiles = (me: number | null, winners: number) =>
  `you ${me == null ? "—" : `${ordinal(me)} percentile`}, winners ${ordinal(winners)}`;

function sides(r: StatRow, bench: Benchmark): string {
  const shown = (s: PlayerReplayStats | null) => {
    const v = s && statValue(r, s);
    return v == null ? "—" : r.format(v);
  };
  return `winners ${shown(bench.winners)}, losers ${shown(bench.losers)}`;
}

function tip(r: StatRow, bench: Benchmark): string {
  const s = bench.stats[r.key]!;
  return `${r.label}: ${percentiles(s.me, s.winners_pct)}; ${sides(r, bench)}, ${direction(s)}`;
}

/** The group's separating stats: the MAX_AXES largest gaps, in the group's order. */
export function groupAxes(group: StatGroup, bench: Benchmark, team: boolean): RadarAxis[] {
  if (!team && group.teamOnly) return [];
  const stat = (r: StatRow) => bench.stats[r.key]!;
  const rows = group.rows.filter((r) => (team || !r.teamOnly) && bench.stats[r.key]?.separates);
  const top = new Set([...rows].sort((a, b) => Math.abs(stat(b).gap) - Math.abs(stat(a).gap)).slice(0, MAX_AXES));
  return rows
    .filter((r) => top.has(r))
    .map((r) => ({ key: r.key, label: r.label, me: stat(r).me, winners: stat(r).winners_pct, tip: tip(r, bench) }));
}

const mean = (xs: number[]) => xs.reduce((a, b) => a + b, 0) / xs.length;

/** One axis per group that has a radar: the mean of its axes, mine and the winners' (spec §4.1). */
export function overviewAxes(groups: StatGroup[], bench: Benchmark, team: boolean): RadarAxis[] {
  return groups.flatMap((g) => {
    const axes = groupAxes(g, bench, team);
    if (axes.length < MIN_AXES) return [];
    const mine = axes.flatMap((a) => (a.me == null ? [] : [a.me]));
    return [{ key: g.name, label: g.name, me: mine.length > 0 ? mean(mine) : null, winners: mean(axes.map((a) => a.winners)) }];
  });
}

/** Progress: a group's every measured stat; those that do not separate are dimmed at 50, like the overview. */
export function progressGroupAxes(group: StatGroup, bench: Benchmark, team: boolean): RadarAxis[] {
  if (!team && group.teamOnly) return [];
  return group.rows
    .filter((r) => (team || !r.teamOnly) && bench.stats[r.key])
    .map((r) => {
      const s = bench.stats[r.key]!;
      return s.separates
        ? { key: r.key, label: r.label, me: s.me, winners: s.winners_pct, tip: tip(r, bench) }
        : { key: r.key, label: r.label, me: 50, winners: 50, dim: true, tip: `${r.label}: ${sides(r, bench)}, no clear difference between winners and losers` };
    });
}

/** Progress: one axis per group with a measured stat; its separating stats averaged, or dimmed at 50 (progress radar spec §3). */
export function progressAxes(groups: StatGroup[], bench: Benchmark, team: boolean): RadarAxis[] {
  return groups.flatMap((g) => {
    if (!team && g.teamOnly) return [];
    const stats = g.rows.filter((r) => team || !r.teamOnly).flatMap((r) => bench.stats[r.key] ?? []);
    if (stats.length === 0) return [];
    const sep = stats.filter((s) => s.separates);
    if (sep.length === 0) {
      return [{ key: g.name, label: g.name, me: 50, winners: 50, dim: true, tip: nothingSeparated(g.name, bench.matches) }];
    }
    const mine = sep.flatMap((s) => (s.me == null ? [] : [s.me]));
    const me = mine.length > 0 ? mean(mine) : null;
    const winners = mean(sep.map((s) => s.winners_pct));
    return [{ key: g.name, label: g.name, me, winners, tip: `${g.name}: ${percentiles(me, winners)}` }];
  });
}
