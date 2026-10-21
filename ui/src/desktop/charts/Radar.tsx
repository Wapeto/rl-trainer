import { useId, useLayoutEffect, useRef, useState, type KeyboardEvent, type PointerEvent, type RefObject } from "react";
import "./charts.css";
import { LABEL_GAP, T_MAX, T_MIN, angle, at, frame, labelLines, radius, sliceAt, valuePoint, type Frame } from "./radarGeometry";

/** One axis of a radar: 0 is the losers' level, 100 the winners' level, in real values. */
export interface RadarAxis {
  key: string;
  label: string;
  /** My position on that scale; null when not measured for me. */
  t: number | null;
  /** How strongly the stat separates winners from losers (|gap| in standard deviations). */
  importance: number;
  /** Read by screen readers; on a dimmed axis, the card says it too. */
  tip?: string;
  /** Drawn faint, with no point: nothing in it separates winners from losers. */
  dim?: boolean;
  /** The real values for the card, already formatted, and their unit ("per 5 min"). */
  you?: string;
  winnersValue?: string;
  losersValue?: string;
  unit?: string;
  lessBetter?: boolean;
}

const AT_WINNERS = 85;
/** The detail panel's width, and the narrowest figure that keeps it beside the chart. */
const PANEL_W = 236;
const SIDE_BY_SIDE = 600;
/** Width assumed before the first measure (and in tests). */
const FALLBACK_W = 640;

type Tone = "red" | "amber" | "green" | "grey";

export function band(t: number): { text: string; tone: Tone } {
  if (t < 0) return { text: "Below the losers' level", tone: "red" };
  if (t < 40) return { text: "Close to the losers' level", tone: "amber" };
  if (t < AT_WINNERS) return { text: `${Math.round(t)}% of the way to the winners`, tone: "amber" };
  if (t <= 115) return { text: "At the winners' level", tone: "green" };
  return { text: "Past the winners' level, more isn't always better", tone: "grey" };
}

/** Bars (1–3) for how strongly a stat separates: small, medium and large effects (|gap| 0.3, 0.5, 0.8). */
export const steps = (g: number) => (g >= 0.8 ? 3 : g >= 0.5 ? 2 : 1);
const WEIGHT = ["", "Matters a little", "Matters", "Matters a lot"];
/** Label strength 0–1, on the same absolute scale as the bars. */
const strength = (g: number) => Math.max(0, Math.min(1, (g - 0.3) / 0.6));

const measured = (axes: RadarAxis[]) => axes.filter((a): a is RadarAxis & { t: number } => !a.dim && a.t != null);

/** The two furthest from the winners, and how many are already there; null when nothing is measured. */
export function takeaway(axes: RadarAxis[]): { work: { label: string; tone: Tone }[]; atWinners: number; total: number } | null {
  const real = measured(axes);
  if (real.length === 0) return null;
  const work = real
    .filter((a) => a.t < AT_WINNERS)
    .sort((a, b) => a.t - b.t)
    .slice(0, 2)
    .map((a) => ({ label: a.label, tone: band(a.t).tone }));
  return { work, atWinners: real.filter((a) => a.t >= AT_WINNERS).length, total: real.length };
}

function verdict(a: RadarAxis): { text: string; tone: Tone | null } {
  if (a.dim) return { text: a.tip ?? "No clear difference between winners and losers", tone: null };
  if (a.t == null) return { text: "Not measured for you", tone: null };
  return band(a.t);
}

const pct = (t: number) => `${((Math.max(T_MIN, Math.min(T_MAX, t)) - T_MIN) / (T_MAX - T_MIN)) * 100}%`;

function Card({ a }: { a: RadarAxis }) {
  const v = verdict(a);
  const values = !a.dim && a.winnersValue != null && a.losersValue != null;
  return (
    <div className="c-radar__card">
      <div className="c-radar__name">
        {a.label}
        {(a.unit || a.lessBetter) && <span>{[a.unit, a.lessBetter && "less is better"].filter(Boolean).join(" · ")}</span>}
      </div>
      {values && (
        <div className="c-radar__vals">
          <span>You</span>
          <b className="c-radar__t-blue">{a.you ?? "—"}</b>
          <span>Winners</span>
          <b className="c-radar__t-green">{a.winnersValue}</b>
          <span>Losers</span>
          <b className="c-radar__t-red">{a.losersValue}</b>
        </div>
      )}
      {!a.dim && (
        <div className="c-radar__strip">
          <div className="c-radar__strip-track" />
          <small style={{ left: pct(0) }}>losers</small>
          <small style={{ left: pct(100) }}>winners</small>
          <i style={{ left: pct(0) }} />
          <i style={{ left: pct(100) }} />
          {a.t != null && <span className="c-radar__strip-me" style={{ left: pct(a.t) }} />}
        </div>
      )}
      <div className={`c-radar__verdict${v.tone ? ` c-radar__t-${v.tone}` : ""}`}>{v.text}</div>
      {!a.dim && <div className="c-radar__weight">{WEIGHT[steps(a.importance)]} for winning in your games</div>}
    </div>
  );
}

/** What the panel shows when no stat is picked: how to use the chart, and its key. */
function Idle({ clickable }: { clickable: boolean }) {
  return (
    <div className="c-radar__idle">
      <p className="c-radar__idle-t">Point at a stat to see your numbers</p>
      <p>Or click the chart and use ← →.{clickable ? " Click a name to open it." : ""}</p>
      <ul className="c-radar__key" aria-hidden="true">
        <li>
          <i className="c-radar__k-me" /> You
        </li>
        <li>
          <i className="c-radar__k-win" /> Winners' level
        </li>
        <li>
          <i className="c-radar__k-lose" /> Losers' level
        </li>
        <li>
          <i className="c-radar__k-bars" /> More bars = matters more
        </li>
      </ul>
    </div>
  );
}

const cardText = (a: RadarAxis) => {
  const values = !a.dim && a.winnersValue != null ? `you ${a.you ?? "not measured"}, winners ${a.winnersValue}, losers ${a.losersValue}. ` : "";
  return `${a.label}: ${values}${verdict(a).text.replace(/\.$/, "")}.`;
};

/** An element's width, kept up to date. */
function useWidth(ref: RefObject<HTMLElement>): number {
  const [w, setW] = useState(0);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const update = () => setW(el.clientWidth);
    update();
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, [ref]);
  return w || FALLBACK_W;
}

/** An axis name, wrapped, placed outward from its spoke end, with its importance bar under it. */
function AxisLabel({ f, ax, i, n, on, onAxis }: { f: Frame; ax: RadarAxis; i: number; n: number; on: boolean; onAxis?: (key: string) => void }) {
  const [x, y] = at(f, i, n, 1, LABEL_GAP);
  const c = Math.cos(angle(i, n));
  const s = Math.sin(angle(i, n));
  const anchor = c > 0.15 ? "start" : c < -0.15 ? "end" : "middle";
  const k = ax.dim ? 0 : strength(ax.importance);
  const size = 12 + k * 2.5;
  const lh = size * 1.2;
  const lines = labelLines(ax.label);
  const block = lines.length * lh + (ax.dim ? 0 : 8);
  // Above a top spoke, below a bottom one, centred on a side one.
  const top = s < -0.3 ? y - block : s > 0.3 ? y : y - block / 2;
  const bw = 7;
  const total = 3 * bw + 6;
  const bx = anchor === "start" ? x : anchor === "end" ? x - total : x - total / 2;
  const by = top + lines.length * lh + 3;
  return (
    <g>
      <text
        textAnchor={anchor}
        className={["c-radar__label", ax.dim && "c-radar__dim", onAxis && "c-radar__click", on && "c-radar__label--on"].filter(Boolean).join(" ")}
        style={{ fontSize: size, fillOpacity: ax.dim ? undefined : 0.55 + k * 0.45 }}
        onClick={onAxis && (() => onAxis(ax.key))}
      >
        {lines.map((l, j) => (
          <tspan key={j} x={x} y={top + (j + 0.5) * lh} dominantBaseline="middle">
            {l}
          </tspan>
        ))}
      </text>
      {!ax.dim && (
        <g className="c-radar__imp">
          {[0, 1, 2].map((j) => (
            <rect key={j} className={j < steps(ax.importance) ? "c-radar__imp-on" : undefined} x={bx + j * (bw + 3)} y={by} width={bw} height={3} rx={1.5} />
          ))}
        </g>
      )}
    </g>
  );
}

/**
 * Where you stand between the losers' and the winners' level on each habit (spec §5.2). Drawn at its
 * real pixel size; the picked stat's card sits in a fixed panel beside the chart (below when narrow),
 * so nothing moves or covers the plot. Hover, keyboard and tap pick a stat.
 */
export function Radar({ label, axes, onAxis }: { label: string; axes: RadarAxis[]; onAxis?: (key: string) => void }) {
  const n = axes.length;
  const id = useId();
  const body = useRef<HTMLDivElement>(null);
  const svg = useRef<SVGSVGElement>(null);
  const [cur, setCur] = useState<number | null>(null);
  const width = useWidth(body);
  const side = width >= SIDE_BY_SIDE;
  const f = frame(side ? width - PANEL_W - 18 : width);
  const ring = (r: number) => axes.map((_, i) => at(f, i, n, r).join(",")).join(" ");
  const take = takeaway(axes);
  // My shape joins only the spokes with a position of mine: a faint axis has none.
  const mine = axes.flatMap((ax, i) => (ax.dim || ax.t == null ? [] : [{ i, key: ax.key, p: valuePoint(f, i, n, ax.t) }]));

  const pick = (e: PointerEvent<HTMLDivElement>) => {
    const r = svg.current?.getBoundingClientRect();
    return r ? sliceAt(e.clientX - r.left - f.cx, e.clientY - r.top - f.cy, n, f.r) : null;
  };
  const onMove = (e: PointerEvent<HTMLDivElement>) => {
    if (e.pointerType === "touch") return; // a finger scrolls; a tap picks
    setCur(pick(e));
  };
  const onDown = (e: PointerEvent<HTMLDivElement>) => {
    if (e.pointerType !== "touch") return;
    const i = pick(e);
    setCur(i === cur ? null : i);
  };
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === "Escape") return setCur(null);
    if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
    e.preventDefault();
    setCur(cur == null ? 0 : (cur + (e.key === "ArrowRight" ? 1 : n - 1)) % n);
  };

  const a = cur == null ? null : axes[cur];
  const rl = radius(0);
  const rw = radius(100);

  return (
    <figure className="c-radar" aria-label={label}>
      {take && (
        <p className="c-radar__take">
          {take.work.length === 0 ? (
            <b className="c-radar__t-green">At the winners' level everywhere</b>
          ) : (
            <>
              Work on{" "}
              {take.work.map((w, i) => (
                <span key={w.label}>
                  {i > 0 && " and "}
                  <b className={`c-radar__t-${w.tone}`}>{w.label}</b>
                </span>
              ))}
              <span className="c-radar__count"> · {take.atWinners} of {take.total} already at the winners' level</span>
            </>
          )}
        </p>
      )}
      <p className="c-radar__how">
        Each spoke runs from how losers play (dashed ring) to how winners play (green ring), in your games. Only habits where they clearly differ get a spoke.
      </p>
      <div ref={body} className={`c-radar__body${side ? " c-radar__body--side" : ""}`}>
        <div
          className="c-radar__plot"
          tabIndex={0}
          role="group"
          aria-label={`${label}. Arrow keys read each stat.`}
          onPointerMove={onMove}
          onPointerDown={onDown}
          onPointerLeave={(e) => e.pointerType !== "touch" && setCur(null)}
          onKeyDown={onKey}
          onBlur={() => setCur(null)}
        >
          <svg ref={svg} width={f.w} height={f.h} viewBox={`0 0 ${f.w} ${f.h}`} aria-hidden="true">
            <defs>
              <radialGradient id={`${id}me`} cx={f.cx} cy={f.cy} r={f.r} gradientUnits="userSpaceOnUse">
                <stop offset="0" stopColor="#7aa7ff" stopOpacity={0.03} />
                <stop offset="1" stopColor="#7aa7ff" stopOpacity={0.32} />
              </radialGradient>
            </defs>
            <polygon className="c-radar__disc" points={ring(1)} />
            <polygon className="c-radar__zone-win" points={ring(rw)} />
            <polygon className="c-radar__zone-lose" points={ring(rl)} />
            {axes.map((ax, i) => {
              const [x, y] = at(f, i, n, 1);
              const [x0, y0] = at(f, i, n, rl);
              return (
                <g key={ax.key}>
                  <line className="c-radar__spoke" x1={f.cx} y1={f.cy} x2={x0} y2={y0} />
                  <line className={`c-radar__spoke c-radar__spoke--out${cur === i ? " c-radar__spoke--on" : ""}`} x1={x0} y1={y0} x2={x} y2={y} />
                </g>
              );
            })}
            <polygon className="c-radar__ring-lose" points={ring(rl)} />
            <polygon className="c-radar__ring-win" points={ring(rw)} />
            {mine.length > 0 && (
              <g className="c-radar__shapes">
                {mine.length > 1 && <polygon className="c-radar__me" fill={`url(#${id}me)`} points={mine.map((m) => m.p.join(",")).join(" ")} />}
                {mine.map((m) => (
                  <circle key={m.key} className="c-radar__dot-me" cx={m.p[0]} cy={m.p[1]} r={cur === m.i ? 6.5 : 4} />
                ))}
              </g>
            )}
            {axes.map((ax, i) => (
              <AxisLabel key={ax.key} f={f} ax={ax} i={i} n={n} on={cur === i} onAxis={onAxis} />
            ))}
          </svg>
        </div>
        <div className="c-radar__panel">{a ? <Card a={a} /> : <Idle clickable={onAxis != null} />}</div>
      </div>
      <p className="c-radar__live" aria-live="polite">
        {a ? cardText(a) : ""}
      </p>
      <ul className="c-radar__alt">
        {axes.map((ax) => (
          <li key={ax.key}>{cardText(ax)}</li>
        ))}
      </ul>
    </figure>
  );
}
