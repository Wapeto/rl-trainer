import { useState } from "react";
import type { DesktopApi } from "../api";
import { Radar } from "../charts/Radar";
import { useLoad } from "../components/useLoad";
import { MIN_AXES, groupAxes, progressAxes } from "../sessions/benchRadar";
import { MIN_POOL_MATCHES, STAT_GROUPS } from "../sessions/replayRows";
import { modeName } from "./format";

/** My last ranked matches in a mode against their winners, by stat group (progress radar spec §2). */
export function WinnersCard({ api, teamSize }: { api: DesktopApi; teamSize: number }) {
  const loaded = useLoad(() => api.modeBenchmark(teamSize), String(teamSize));
  const [open, setOpen] = useState<string | null>(null);
  const bench = loaded.data;
  const mode = modeName(teamSize);
  if (loaded.error && !bench) return <p className="d-ins__note">{loaded.error}</p>;
  if (!bench) return null;
  const axes = progressAxes(STAT_GROUPS, bench, teamSize > 1);
  const toggle = (name: string) => setOpen(open === name ? null : name);
  const group = STAT_GROUPS.find((g) => g.name === open);
  const detail = group ? groupAxes(group, bench, teamSize > 1) : [];
  return (
    <section className="d-kpi glass p-winners" aria-label="Against the winners">
      <div className="d-k">Against the winners</div>
      {bench.winners == null || axes.length < MIN_AXES ? (
        <p className="d-ins__note">
          {bench.winners == null
            ? `The winners' benchmark appears after ${MIN_POOL_MATCHES} ranked replays in ${mode} (${bench.matches} so far).`
            : `Too few stats measured in your last ${bench.matches} ranked ${mode}s.`}
        </p>
      ) : (
        <>
          <Radar label="You against the winners, by category" axes={axes} onAxis={toggle} />
          <p className="d-ins__note">{`Your last ${bench.matches} ranked ${mode}s against their winners.`}</p>
          <div className="p-winners__groups">
            {axes.map((a) => (
              <button key={a.key} type="button" aria-pressed={open === a.key} onClick={() => toggle(a.key)}>
                {a.label}
              </button>
            ))}
          </div>
          {group &&
            (detail.length >= MIN_AXES ? (
              <Radar label={`${group.name}: you against the winners`} axes={detail} />
            ) : (
              <p className="d-ins__note">{`Too few ${group.name} stats separate winners from losers to draw a radar.`}</p>
            ))}
        </>
      )}
    </section>
  );
}
