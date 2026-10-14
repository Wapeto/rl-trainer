import { useState } from "react";
import { axisDates, axisValue, domain, scale } from "./scale";
import { useWidth } from "./useWidth";
import "./charts.css";

export interface ChartPoint {
  at: number;
  value: number;
  /** The session a click opens. */
  sessionId: number | null;
  /** Drawn as a ring (too few attempts to trust). */
  hollow?: boolean;
  /** A bar along the bottom, scaled to the largest (attempts). */
  bar?: number;
  /** Tooltip lines. */
  tip: string[];
}

export interface ChartSeries {
  label: string;
  color: string;
  points: ChartPoint[];
}

export interface LineChartProps {
  label: string;
  series: ChartSeries[];
  height: number;
  unit?: string;
  /** A dashed level line: the pack's target, or 50% for the win rate. */
  reference?: number | null;
  /** A dashed line through the first series, one value per point; null skips the point. */
  average?: (number | null)[];
  /** No axes (the small charts). */
  compact?: boolean;
  onOpenSession?: (sessionId: number) => void;
}

const FULL = { left: 34, right: 8, top: 8, bottom: 16 };
const SMALL = { left: 4, right: 4, top: 4, bottom: 4 };
const BAR_SHARE = 0.3;


export function LineChart(props: LineChartProps) {
  const { label, series, height, unit = "", reference = null, average, compact = false, onOpenSession } = props;
  const [ref, width] = useWidth<HTMLDivElement>();
  const [hover, setHover] = useState<ChartPoint | null>(null);
  const all = series.flatMap((s) => s.points);
  if (all.length === 0) {
    return (
      <div className="c-chart c-chart--empty" style={{ height }}>
        No data yet
      </div>
    );
  }
  const pad = compact ? SMALL : FULL;
  const times = all.map((p) => p.at);
  const t0 = Math.min(...times);
  const t1 = Math.max(...times);
  const [lo, hi] = domain([...all.map((p) => p.value), ...(reference == null ? [] : [reference])]);
  const x = scale(t0, t1, pad.left, width - pad.right);
  const y = scale(lo, hi, height - pad.bottom, pad.top);
  const plotHeight = height - pad.top - pad.bottom;
  const first = series[0]?.points ?? [];
  const barMax = Math.max(1, ...first.map((p) => p.bar ?? 0));
  const averagePoints = average
    ? first.flatMap((p, i) => (average[i] == null ? [] : [`${x(p.at)},${y(average[i] as number)}`])).join(" ")
    : "";
  return (
    <div className="c-chart" ref={ref} style={{ height }}>
      <svg width={width} height={height} role="img" aria-label={label}>
        {!compact &&
          [lo, (lo + hi) / 2, hi].map((v) => (
            <g key={v}>
              <line className="c-grid" x1={pad.left} x2={width - pad.right} y1={y(v)} y2={y(v)} />
              <text className="c-axis" x={0} y={y(v) + 3}>
                {axisValue(v, unit)}
              </text>
            </g>
          ))}
        {!compact &&
          axisDates(t0, t1).map((t) => (
            <text key={t.at} className="c-axis" x={x(t.at)} y={height - 3} textAnchor={t.anchor}>
              {t.label}
            </text>
          ))}
        {reference != null && <line className="c-ref" x1={pad.left} x2={width - pad.right} y1={y(reference)} y2={y(reference)} />}
        {first.map((p) =>
          p.bar ? (
            <rect
              key={`bar${p.at}`}
              className="c-bar"
              x={x(p.at) - 3}
              width={6}
              y={height - pad.bottom - (p.bar / barMax) * plotHeight * BAR_SHARE}
              height={(p.bar / barMax) * plotHeight * BAR_SHARE}
            />
          ) : null,
        )}
        {averagePoints && <polyline className="c-avg" points={averagePoints} />}
        {series.map((s) => (
          <g key={s.label}>
            <polyline fill="none" stroke={s.color} strokeWidth={1.8} points={s.points.map((p) => `${x(p.at)},${y(p.value)}`).join(" ")} />
            {s.points.map((p) => (
              <circle
                key={p.at}
                className={p.hollow ? "c-pt c-pt--hollow" : "c-pt"}
                cx={x(p.at)}
                cy={y(p.value)}
                r={compact ? 2 : 3.2}
                stroke={s.color}
                fill={p.hollow ? "var(--d-bg)" : s.color}
                aria-label={p.tip.join(" · ")}
                onMouseEnter={() => setHover(p)}
                onMouseLeave={() => setHover(null)}
                onClick={(e) => {
                  if (p.sessionId == null || !onOpenSession) return;
                  e.stopPropagation();
                  onOpenSession(p.sessionId);
                }}
              />
            ))}
          </g>
        ))}
      </svg>
      {hover && (
        <div className="c-tip" role="tooltip" style={{ left: x(hover.at), top: y(hover.value) }}>
          {hover.tip.map((line) => (
            <div key={line}>{line}</div>
          ))}
        </div>
      )}
    </div>
  );
}
