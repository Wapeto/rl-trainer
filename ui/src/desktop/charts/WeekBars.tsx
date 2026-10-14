import { longDuration } from "@shared/format";
import { useState } from "react";
import type { WeekTotals } from "./weeks";

const PARTS = [
  { key: "training", label: "Training", color: "var(--green)" },
  { key: "freeplay", label: "Free play", color: "var(--amber)" },
  { key: "ranked", label: "Ranked", color: "var(--d-blue)" },
  { key: "other", label: "Casual & other", color: "var(--d-dim)" },
] as const;

const total = (w: WeekTotals) => w.training + w.freeplay + w.ranked + w.other;
const day = (ms: number) => new Date(ms).toLocaleDateString("en-GB", { day: "numeric", month: "short" });

/** Stacked bars of play time per week (training, free play, ranked, casual and other), with a
 * legend of each kind's time over the shown weeks. */
export function WeekBars({ weeks, height = 96 }: { weeks: WeekTotals[]; height?: number }) {
  const [hover, setHover] = useState<number | null>(null);
  const most = Math.max(1, ...weeks.map(total));
  const shown = hover == null ? null : weeks[hover];
  return (
    <div className="c-weekchart">
      <div className="c-weeks" style={{ height }} role="img" aria-label="Time per week" onMouseLeave={() => setHover(null)}>
        {weeks.map((w, i) => (
          <div key={w.start} className={`c-week${i === hover ? " c-week--on" : ""}`} onMouseEnter={() => setHover(i)}>
            <span className="c-sr">{`Week of ${day(w.start)} · ${longDuration(total(w))}`}</span>
            {PARTS.map(({ key, color }) => (
              <span key={key} className="c-week__part" style={{ height: `${(w[key] / most) * 100}%`, background: color }} />
            ))}
          </div>
        ))}
        {shown && hover != null && (
          <div
            className={`c-tip c-tip--in${hover >= weeks.length / 2 ? " c-tip--leftward" : ""}`}
            role="tooltip"
            style={{ left: `${((hover + 0.5) / weeks.length) * 100}%`, top: 0 }}
          >
            <div>{longDuration(total(shown))}</div>
            <div>Week of {day(shown.start)}</div>
            {PARTS.filter(({ key }) => shown[key] > 0).map(({ key, label, color }) => (
              <div key={key} className="c-tip__row">
                <i className="c-swatch" style={{ background: color }} />
                {label} {longDuration(shown[key])}
              </div>
            ))}
          </div>
        )}
      </div>
      {weeks.length > 0 && (
        <div className="c-weekaxis" aria-hidden>
          <span>{day(weeks[0].start)}</span>
          {weeks.length > 1 && <span>{day(weeks[weeks.length - 1].start)}</span>}
        </div>
      )}
      <ul className="c-legend" aria-label="Kinds of play">
        {PARTS.map(({ key, label, color }) => (
          <li key={key}>
            <i className="c-swatch" style={{ background: color }} />
            <span>{label}</span>
            <b>{longDuration(weeks.reduce((sum, w) => sum + w[key], 0))}</b>
          </li>
        ))}
      </ul>
    </div>
  );
}
