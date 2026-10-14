import { longDuration } from "@shared/format";
import type { PackProgress, ProgressRange } from "@shared/generated/types";
import { PROGRESS } from "@shared/glossary";
import { Hint } from "@shared/hint/Hint";
import { ChartCard } from "../charts/ChartCard";
import { useLoad } from "../components/useLoad";
import { percent, plural } from "../format";
import { rateTone } from "../sessions/timeline";
import { changeTone, pointsChange, shortDate } from "./format";
import type { ProgressPageProps } from "./ProgressPage";

type PackProgressViewProps = ProgressPageProps & { range: ProgressRange; packId: number };

const rateColor = (p: PackProgress) => (rateTone(p.rate, p.pack.target_rate) === "green" ? "#6fe3a9" : "#f2b46b");

function chart(p: PackProgress, onOpenSession: (id: number) => void) {
  const color = rateColor(p);
  return {
    series: [
      {
        label: "Make rate",
        color,
        points: p.points.map((x) => ({
          at: x.at,
          value: x.rate * 100,
          sessionId: x.session_id,
          hollow: x.thin,
          bar: x.attempts,
          tip: [`${percent(x.rate)} · ${x.makes} of ${x.attempts}`, `${shortDate(x.at)} · ${longDuration(x.ms)}`],
        })),
      },
    ],
    reference: p.pack.target_rate * 100,
    average: p.points.map((x) => (x.average == null ? null : x.average * 100)),
    unit: "%",
    onOpenSession,
  };
}

/** What the chart's lines, hollow points and bars are, drawn like the chart draws them. */
function Legend({ p }: { p: PackProgress }) {
  return (
    <ul className="p-legend" aria-label="Chart legend">
      <li>
        <svg width="18" height="8" aria-hidden>
          <line className="c-ref" x1="0" x2="18" y1="4" y2="4" />
        </svg>
        {`Target ${percent(p.pack.target_rate)}`}
      </li>
      <li>
        <svg width="18" height="8" aria-hidden>
          <line className="c-avg" x1="0" x2="18" y1="4" y2="4" />
        </svg>
        Average of the last 3 sessions
      </li>
      <li>
        <svg width="10" height="10" aria-hidden>
          <circle cx="5" cy="5" r="3.5" fill="none" stroke={rateColor(p)} strokeWidth="1.5" />
        </svg>
        Under 10 attempts
      </li>
      <li>
        <svg width="8" height="10" aria-hidden>
          <rect className="c-bar" x="1" y="1" width="6" height="9" />
        </svg>
        Attempts per session
      </li>
    </ul>
  );
}

export function PackProgressView({ api, range, reload, packId, onOpenPack, onOpenSession }: PackProgressViewProps) {
  const loaded = useLoad(() => api.progressPack(packId, range), `${packId}|${range}|${reload}`);
  if (loaded.data === undefined) return <p className="d-empty">{loaded.error ?? "Loading…"}</p>;
  const p = loaded.data;
  if (p == null) return <p className="d-empty">This pack no longer exists.</p>;
  const first = p.points[0];
  const latest = p.points[p.points.length - 1];
  const tone = (rate: number | null | undefined) => (rateTone(rate ?? null, p.pack.target_rate) === "green" ? "d-g" : "d-a");
  return (
    <>
      <div className="d-hd">
        <div>
          <h1>{p.pack.name}</h1>
          <div className="d-meta">
            <span className="d-code">{p.pack.code}</span>
          </div>
        </div>
        <button type="button" className="d-btn" onClick={() => onOpenPack(p.pack.id)}>
          Open in Packs ›
        </button>
      </div>
      <div className="d-kpis">
        <div className="d-kpi glass">
          <div className="d-k">
            <Hint gloss={PROGRESS.latest_session}>Latest session</Hint>
          </div>
          <div className={`d-kpi__v ${tone(latest?.rate)}`}>
            {percent(latest?.rate)}
            <small className={changeTone(p.latest_change)}>{pointsChange(p.latest_change)}</small>
          </div>
          <div className="p-sub">vs first in range: {percent(first?.rate)}</div>
        </div>
        <div className="d-kpi glass">
          <div className="d-k">
            <Hint gloss={PROGRESS.over_range}>Over the range</Hint>
          </div>
          <div className={`d-kpi__v ${tone(p.rate)}`}>{percent(p.rate)}</div>
          <div className="p-sub">target {percent(p.pack.target_rate)}</div>
        </div>
        <div className="d-kpi glass">
          <div className="d-k">
            <Hint gloss={PROGRESS.best_session}>Best session</Hint>
          </div>
          <div className="d-kpi__v">{percent(p.best?.rate)}</div>
          <div className="p-sub">{p.best ? `${shortDate(p.best.at)} · ${p.best.attempts} attempts` : "10+ attempts needed"}</div>
        </div>
        <div className="d-kpi glass">
          <div className="d-k">
            <Hint gloss={PROGRESS.volume}>Volume</Hint>
          </div>
          <div className="d-kpi__v">
            {p.attempts}
            <small>attempts</small>
          </div>
          <div className="p-sub">
            {plural(p.points.length, "session")} · {longDuration(p.ms)}
          </div>
        </div>
      </div>
      <div className="p-grid2" style={{ gridTemplateColumns: "1fr" }}>
        <ChartCard title="Make rate" value={percent(p.rate)} gloss={PROGRESS.pack_chart} compact={false} height={230} chart={chart(p, onOpenSession)} />
      </div>
      <Legend p={p} />
      <p className="p-note">Sessions with under 10 attempts on the pack are drawn as hollow points and left out of the average line.</p>
    </>
  );
}
