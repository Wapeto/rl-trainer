import type { BenchStat, Benchmark } from "@shared/generated/types";
import type { RadarAxis } from "../charts/Radar";
import { type StatGroup, type StatRow, statValue } from "./replayRows";

/** Most axes on a group radar, and fewest worth drawing (winners' benchmark spec §4.2). */
export const MAX_AXES = 6;
export const MIN_AXES = 3;

/** Which way the winners lean on a stat, when they clearly differ from the losers. */
export function direction(s: BenchStat | undefined): string | null {
  return s?.separates ? (s.gap > 0 ? "more is better" : "less is better") : null;
}

/** Why a group has no radar, or a dimmed Progress axis. */
export function nothingSeparated(group: string, matches: number): string {
  return `Nothing in ${group} separated winners from losers in your last ${matches} matches.`;
}

const UNITS: Record<StatRow["kind"], string> = { raw: "game average", per5: "per 5 min", share: "% of the time", ofTouches: "% of touches" };

const mean = (xs: number[]) => xs.reduce((a, b) => a + b, 0) / xs.length;

/** A separating stat as an axis: the losers' and winners' means in real values, and mine (spec §5.2). */
function axisOf(r: StatRow, bench: Benchmark): RadarAxis | null {
  const s = bench.stats[r.key];
  const read = (m: Benchmark["mine"]) => (m ? statValue(r, m) : null);
  const [l, w, me] = [read(bench.losers), read(bench.winners), read(bench.mine)];
  if (!s?.separates || l == null || w == null || l === w) return null;
  return {
    key: r.key,
    label: r.label,
    t: me == null ? null : ((me - l) / (w - l)) * 100,
    importance: Math.abs(s.gap),
    you: me == null ? undefined : r.format(me),
    winnersValue: r.format(w),
    losersValue: r.format(l),
    unit: UNITS[r.kind],
    lessBetter: w < l,
  };
}

/** Every separating stat of a group, in the group's order; none for a team-only group in 1v1. */
function separating(group: StatGroup, bench: Benchmark, team: boolean): RadarAxis[] {
  if (!team && group.teamOnly) return [];
  return group.rows.filter((r) => team || !r.teamOnly).flatMap((r) => axisOf(r, bench) ?? []);
}

/** The group's separating stats: the MAX_AXES that matter most, in the group's order. */
export function groupAxes(group: StatGroup, bench: Benchmark, team: boolean): RadarAxis[] {
  const all = separating(group, bench, team);
  const top = new Set([...all].sort((a, b) => b.importance - a.importance).slice(0, MAX_AXES));
  return all.filter((a) => top.has(a));
}

/** One axis from a group's stats: their mean position and importance. */
function groupAxis(name: string, axes: RadarAxis[]): RadarAxis {
  const ts = axes.flatMap((a) => (a.t == null ? [] : [a.t]));
  return { key: name, label: name, t: ts.length > 0 ? mean(ts) : null, importance: mean(axes.map((a) => a.importance)), unit: axes.length === 1 ? "from 1 stat" : `average of ${axes.length} stats` };
}

/** One axis per group with a separating stat, the same rule as Progress without its dimmed groups (spec §4.1). */
export function overviewAxes(groups: StatGroup[], bench: Benchmark, team: boolean): RadarAxis[] {
  return groups.flatMap((g) => {
    const axes = separating(g, bench, team);
    return axes.length === 0 ? [] : [groupAxis(g.name, axes)];
  });
}

/** Progress: one axis per group with a measured stat; a group where nothing separates is dimmed. */
export function progressAxes(groups: StatGroup[], bench: Benchmark, team: boolean): RadarAxis[] {
  return groups.flatMap((g): RadarAxis[] => {
    if (!team && g.teamOnly) return [];
    if (!g.rows.some((r) => (team || !r.teamOnly) && bench.stats[r.key])) return [];
    const axes = separating(g, bench, team);
    if (axes.length === 0) return [{ key: g.name, label: g.name, t: null, importance: 0, dim: true, tip: nothingSeparated(g.name, bench.matches) }];
    return [groupAxis(g.name, axes)];
  });
}
