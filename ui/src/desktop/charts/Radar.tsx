import "./charts.css";

/** One axis of a radar: 0–100, outward is better. */
export interface RadarAxis {
  key: string;
  label: string;
  me: number | null;
  winners: number;
  /** Shown on hover and read by screen readers. */
  tip?: string;
  /** Drawn faint: nothing in it separates winners from losers. */
  dim?: boolean;
}

const W = 360;
const H = 260;
const CX = W / 2;
const CY = H / 2;
const R = 80;
const LABEL_AT = 112;
const RINGS = [25, 50, 75, 100];

/** Where `value` (0–100, or more for labels) sits on axis `i` of `n`, from the top, clockwise. */
export function radarPoint(i: number, n: number, value: number): [number, number] {
  const a = -Math.PI / 2 + (i * 2 * Math.PI) / n;
  const r = (value / 100) * R;
  return [Math.round((CX + r * Math.cos(a)) * 10) / 10, Math.round((CY + r * Math.sin(a)) * 10) / 10];
}

const points = (values: number[]) =>
  values.map((v, i) => radarPoint(i, values.length, Math.max(0, Math.min(100, v))).join(",")).join(" ");

/** You against the winners on 0–100 axes, outward is better (winners' benchmark spec §4). */
export function Radar({ label, axes, onAxis }: { label: string; axes: RadarAxis[]; onAxis?: (key: string) => void }) {
  const n = axes.length;
  return (
    <figure className="c-radar" aria-label={label}>
      <svg viewBox={`0 0 ${W} ${H}`} aria-hidden="true">
        {RINGS.map((ring) => (
          <polygon key={ring} className="c-radar__ring" points={points(axes.map(() => ring))} />
        ))}
        {axes.map((a, i) => {
          const [x, y] = radarPoint(i, n, 100);
          return <line key={a.key} className="c-radar__ring" x1={CX} y1={CY} x2={x} y2={y} />;
        })}
        <polygon className="c-radar__winners" points={points(axes.map((a) => a.winners))} />
        <polygon className="c-radar__me" points={points(axes.map((a) => a.me ?? 0))} />
        {axes.map((a, i) => {
          const [x, y] = radarPoint(i, n, LABEL_AT);
          const anchor = x > CX + 1 ? "start" : x < CX - 1 ? "end" : "middle";
          return (
            <text
              key={a.key}
              x={x}
              y={y}
              textAnchor={anchor}
              dominantBaseline="middle"
              className={[a.dim && "c-radar__dim", onAxis && "c-radar__click"].filter(Boolean).join(" ") || undefined}
              onClick={onAxis && (() => onAxis(a.key))}
            >
              {a.tip && <title>{a.tip}</title>}
              {a.label}
            </text>
          );
        })}
      </svg>
      <figcaption className="c-radar__key">
        <i className="c-radar__k-me" /> You <i className="c-radar__k-winners" /> Winners
      </figcaption>
      <ul className="c-radar__alt">
        {axes.map((a) => (
          <li key={a.key}>{a.dim ? `${a.label} — ${a.tip ?? ""}` : `${a.label}: you ${a.me == null ? "—" : Math.round(a.me)}, winners ${Math.round(a.winners)}${a.tip ? ` — ${a.tip}` : ""}`}</li>
        ))}
      </ul>
    </figure>
  );
}
