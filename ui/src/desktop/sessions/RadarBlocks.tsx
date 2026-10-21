import type { Benchmark } from "@shared/generated/types";
import { Radar } from "../charts/Radar";
import { MIN_AXES, groupAxes, nothingSeparated, overviewAxes } from "./benchRadar";
import { STAT_GROUPS, type StatGroup } from "./replayRows";

/** A group's radar, or why it has none; nothing when no stat of the group was measured (spec §4.2). */
export function GroupRadar({ group, bench, team }: { group: StatGroup; bench: Benchmark; team: boolean }) {
  const axes = groupAxes(group, bench, team);
  if (axes.length >= MIN_AXES) return <Radar label={`${group.name}: you against the winners`} axes={axes} />;
  if (!group.rows.some((r) => bench.stats[r.key])) return null;
  return <p className="d-ins__note">{nothingSeparated(group.name, bench.matches)}</p>;
}

/** One axis per group radar, beside what stood out (spec §4.1). */
export function OverviewRadar({ bench, team, mode }: { bench: Benchmark; team: boolean; mode: string }) {
  const axes = overviewAxes(STAT_GROUPS, bench, team);
  if (axes.length < MIN_AXES) return null;
  return (
    <div className="d-ins__radar">
      <Radar label="You against the winners, by category" axes={axes} />
      <p className="d-ins__note">{`Against the winners of your last ${bench.matches} ranked ${mode}s.`}</p>
    </div>
  );
}
