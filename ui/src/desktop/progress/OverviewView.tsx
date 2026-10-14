import { longDuration } from "@shared/format";
import type { OverviewProgress, ProgressRange } from "@shared/generated/types";
import { PROGRESS } from "@shared/glossary";
import { Hint } from "@shared/hint/Hint";
import { ChartCard } from "../charts/ChartCard";
import type { ChartSeries } from "../charts/LineChart";
import { Sparkline } from "../charts/Sparkline";
import { WeekBars } from "../charts/WeekBars";
import { weekBuckets } from "../charts/weeks";
import { useLoad } from "../components/useLoad";
import { percent, plural } from "../format";
import { changeTone, MODE_COLORS, modeName, pointsChange, shortDate, signed, winPct } from "./format";
import type { ProgressPageProps, Selection } from "./ProgressPage";

type OverviewViewProps = ProgressPageProps & { range: ProgressRange; onSelect: (s: Selection) => void };

function mmrSeries(o: OverviewProgress): ChartSeries[] {
  return o.mmr.map((m) => ({
    label: modeName(m.team_size),
    color: MODE_COLORS[m.team_size],
    points: m.points.map((p) => ({ at: p.at, value: p.mmr, sessionId: p.session_id, tip: [`${modeName(m.team_size)} · ${p.mmr}`, shortDate(p.at)] })),
  }));
}

/** "3h 55m" with the units set small, so the numbers read first. */
function HoursMinutes({ ms }: { ms: number }) {
  const minutes = Math.max(0, Math.round(ms / 60_000));
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  if (hours === 0) {
    return (
      <>
        {rest}
        <small className="p-unit">m</small>
      </>
    );
  }
  return (
    <>
      {hours}
      <small className="p-unit">h</small> {String(rest).padStart(2, "0")}
      <small className="p-unit">m</small>
    </>
  );
}

export function OverviewView({ api, range, reload, now, onSelect, onOpenSession }: OverviewViewProps) {
  const loaded = useLoad(() => api.progressOverview(range), `${range}|${reload}`);
  const o = loaded.data;
  if (!o) return <p className="d-empty">{loaded.error ?? "Loading…"}</p>;
  const from = o.start ?? o.time[0]?.at ?? now;
  const won = winPct(o.wins, o.losses);
  return (
    <>
      <div className="d-hd">
        <div>
          <h1>Overview</h1>
          <p className="p-sub">
            {shortDate(from)} – {shortDate(now)} · {plural(o.sessions, "session")} · {longDuration(o.played_ms)}
          </p>
        </div>
      </div>
      <div className="d-kpis">
        <div className="d-kpi glass">
          <div className="d-k">
            <Hint gloss={PROGRESS.ranked}>Ranked</Hint>
          </div>
          <div className="d-kpi__v">
            <span className="d-g">{o.wins}W</span> <span className="d-r">{o.losses}L</span>
          </div>
          <div className="p-sub">
            {won ?? "—"}% won · {plural(o.ranked_matches, "match", "matches")}
          </div>
        </div>
        <div className="d-kpi glass">
          <div className="d-k">
            <Hint gloss={PROGRESS.make_rate}>Make rate</Hint>
          </div>
          <div className="d-kpi__v">
            {percent(o.make_rate)}
            <small className={changeTone(o.make_rate_change)}>{pointsChange(o.make_rate_change)}</small>
          </div>
          <div className="p-sub">
            {o.attempts} attempts · {plural(o.packs_played, "pack")}
          </div>
        </div>
        <div className="d-kpi glass">
          <div className="d-k">
            <Hint gloss={PROGRESS.packs_at_target}>Packs at target</Hint>
          </div>
          <div className="d-kpi__v">
            {o.packs_at_target}
            <small>of {o.packs_played}</small>
          </div>
        </div>
        <div className="d-kpi glass">
          <div className="d-k">
            <Hint gloss={PROGRESS.played}>Played</Hint>
          </div>
          <div className="d-kpi__v">
            <HoursMinutes ms={o.played_ms} />
          </div>
          <div className="p-sub">
            {longDuration(o.per_week_ms)} a week · {plural(o.sessions, "session")}
          </div>
        </div>
      </div>
      {o.modes.length === 0 ? (
        <p className="d-empty">No ranked matches in this range</p>
      ) : (
        <>
          <div className="p-grid2" style={{ gridTemplateColumns: "1fr" }}>
            <ChartCard title="MMR" value="" gloss={PROGRESS.mmr_chart} compact={false} height={150} chart={{ series: mmrSeries(o), onOpenSession }} />
          </div>
          <div className="p-grid3">
            {o.modes.map((m) => (
              <button key={m.team_size} type="button" className="c-card glass p-mode" aria-label={`Open ranked ${modeName(m.team_size)}`} onClick={() => onSelect({ kind: "mode", teamSize: m.team_size })}>
                <span style={{ flex: 1 }}>
                  <span className="d-k">Ranked {modeName(m.team_size)}</span>
                  <span className="d-kpi__v" style={{ display: "block" }}>
                    {m.mmr_latest ?? "—"} <small className={changeTone(m.mmr_change)}>{signed(m.mmr_change)}</small>
                  </span>
                  <span className="p-sub">
                    {m.wins}W {m.losses}L · {winPct(m.wins, m.losses) ?? "—"}%
                  </span>
                </span>
                <Sparkline values={m.mmr_spark} color={MODE_COLORS[m.team_size]} width={90} height={34} />
              </button>
            ))}
          </div>
        </>
      )}
      <div className="p-grid2" style={{ gridTemplateColumns: "1fr 1fr" }}>
        <section className="c-card glass" aria-label="Packs that moved">
          <div className="c-card__t">Packs that moved</div>
          {o.attempts === 0 ? (
            <p className="d-empty">No training in this range</p>
          ) : (
            <div className="p-movers">
              {o.movers.map((m) => (
                <div key={m.pack.id}>
                  <span className="p-name">{m.pack.name}</span>
                  <span className="d-m">
                    {percent(m.from)} → {percent(m.to)}
                  </span>
                  <b className={changeTone(m.to - m.from)}>{signed((m.to - m.from) * 100)}</b>
                </div>
              ))}
              <p className="p-note">Only packs with 2+ sessions in the range.</p>
            </div>
          )}
        </section>
        <section className="c-card glass" aria-label="Time per week">
          <div className="c-card__t">Time per week</div>
          <WeekBars weeks={weekBuckets(o.time, from, now)} />
        </section>
      </div>
    </>
  );
}
