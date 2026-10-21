import { useId, useLayoutEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import "./charts.css";
import { CX, CY, H, R, T_MAX, T_MIN, W, at, dockCard, radius, sliceAt, valuePoint } from "./radarGeometry";

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
  /** Drawn faint: nothing in it separates winners from losers. */
  dim?: boolean;
  /** The real values for the card, already formatted, and their unit ("per 5 min"). */
  you?: string;
  winnersValue?: string;
  losersValue?: string;
  unit?: string;
  lessBetter?: boolean;
}

/** Where on the scale a dimmed axis is drawn: halfway, as a placeholder with no point. */
const DIM_T = 50;
const AT_WINNERS = 85;

type Tone = "red" | "amber" | "green" | "grey";

export function band(t: number): { text: string; tone: Tone } {
  if (t < 0) return { text: "Below the losers' level", tone: "red" };
  if (t < 40) return { text: "Close to the losers' level", tone: "amber" };
  if (t < AT_WINNERS) return { text: `${Math.round(t)}% of the way to the winners`, tone: "amber" };
  if (t <= 115) return { text: "At the winners' level", tone: "green" };
  return { text: "Past the winners' level, more isn't always better", tone: "grey" };
}

const weight = (g: number) => (g >= 0.65 ? "Matters a lot" : g >= 0.45 ? "Matters" : "Matters a little");

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
    <>
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
      {!a.dim && <div className="c-radar__weight">{weight(a.importance)} for winning in your games</div>}
    </>
  );
}

const cardText = (a: RadarAxis) => {
  const values = !a.dim && a.winnersValue != null ? `you ${a.you ?? "not measured"}, winners ${a.winnersValue}, losers ${a.losersValue}. ` : "";
  return `${a.label}: ${values}${verdict(a).text.replace(/\.$/, "")}.`;
};

/** Brightness and bar steps of an axis name: how much it matters next to the other axes. */
function strengths(axes: RadarAxis[]): number[] {
  const imp = axes.filter((a) => !a.dim).map((a) => a.importance);
  const lo = Math.min(...imp);
  const span = Math.max(...imp) - lo || 1;
  return axes.map((a) => (a.dim ? 0 : (a.importance - lo) / span));
}

/**
 * Where you stand between the losers' and the winners' level on each habit (spec §5.2): slice hover,
 * a card docked outside the plot, keyboard and tap.
 */
export function Radar({ label, axes, onAxis }: { label: string; axes: RadarAxis[]; onAxis?: (key: string) => void }) {
  const n = axes.length;
  const id = useId();
  const plot = useRef<HTMLDivElement>(null);
  const card = useRef<HTMLDivElement>(null);
  const [cur, setCur] = useState<number | null>(null);
  const [dock, setDock] = useState<{ x: number; y: number; w: number; h: number } | null>(null);
  const tOf = (a: RadarAxis) => (a.dim ? DIM_T : a.t);
  const pt = (i: number, t: number) => valuePoint(i, n, t);
  const ring = (r: number) => axes.map((_, i) => at(i, n, r).join(",")).join(" ");
  const strong = strengths(axes);
  const take = takeaway(axes);
  const anyMe = axes.some((a) => tOf(a) != null);

  useLayoutEffect(() => {
    if (cur == null || !card.current || !plot.current) return setDock(null);
    const k = plot.current.clientWidth / W || 1;
    const w = card.current.offsetWidth / k;
    const h = card.current.offsetHeight / k;
    const box = dockCard(cur, n, w, h);
    setDock(box && { ...box, w, h });
  }, [cur, n]);

  const pick = (e: PointerEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    const k = r.width / W || 1;
    return sliceAt((e.clientX - r.left) / k - CX, (e.clientY - r.top) / k - CY, n);
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
  const end = cur == null ? null : at(cur, n, 1);
  const leader =
    end && dock
      ? `${end[0]},${end[1]} ${Math.max(dock.x, Math.min(end[0], dock.x + dock.w))},${Math.max(dock.y, Math.min(end[1], dock.y + dock.h))}`
      : null;
  const rl = radius(0);
  const rw = radius(100);

  return (
    <figure className="c-radar" aria-label={label}>
      <div className="c-radar__hd">
        <div>
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
                  ))}{" "}
                  · {take.atWinners} of {take.total} already at the winners' level
                </>
              )}
            </p>
          )}
          <p className="c-radar__how">Each spoke runs from how losers play (dashed ring) to how winners play (outer ring), in your games.</p>
        </div>
        <div className="c-radar__key" aria-hidden="true">
          <span>
            <i className="c-radar__k-me" /> You
          </span>
          <span>
            <i className="c-radar__k-win" /> Winners' level
          </span>
          <span>
            <i className="c-radar__k-lose" /> Losers' level
          </span>
          <span>
            <i className="c-radar__k-bars" /> Bars = matters more
          </span>
        </div>
      </div>
      <div
        ref={plot}
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
        <svg viewBox={`0 0 ${W} ${H}`} aria-hidden="true">
          <defs>
            <radialGradient id={`${id}me`} cx={CX} cy={CY} r={R} gradientUnits="userSpaceOnUse">
              <stop offset="0" stopColor="#7aa7ff" stopOpacity={0.03} />
              <stop offset="1" stopColor="#7aa7ff" stopOpacity={0.32} />
            </radialGradient>
          </defs>
          <polygon className="c-radar__disc" points={ring(1)} />
          <polygon className="c-radar__zone-win" points={ring(rw)} />
          <polygon className="c-radar__zone-lose" points={ring(rl)} />
          {axes.map((ax, i) => {
            const [x, y] = at(i, n, 1);
            const [x0, y0] = at(i, n, rl);
            return (
              <g key={ax.key}>
                <line className="c-radar__spoke" x1={CX} y1={CY} x2={x0} y2={y0} />
                <line className={`c-radar__spoke c-radar__spoke--out${cur === i ? " c-radar__spoke--on" : ""}`} x1={x0} y1={y0} x2={x} y2={y} />
              </g>
            );
          })}
          <polygon className="c-radar__ring-lose" points={ring(rl)} />
          <polygon className="c-radar__ring-win" points={ring(rw)} />
          {anyMe && (
            <g className="c-radar__shapes">
              <polygon className="c-radar__me" fill={`url(#${id}me)`} points={axes.map((ax, i) => pt(i, tOf(ax) ?? T_MIN).join(",")).join(" ")} />
              {axes.map((ax, i) => {
                const t = tOf(ax);
                if (ax.dim || t == null) return null;
                const [x, y] = pt(i, t);
                return <circle key={ax.key} className="c-radar__dot-me" cx={x} cy={y} r={cur === i ? 7 : 4} />;
              })}
            </g>
          )}
          {leader && <polyline className="c-radar__leader" points={leader} />}
          {axes.map((ax, i) => {
            const [x, y] = at(i, n, 1 + 20 / R);
            const anchor = x > CX + 1 ? "start" : x < CX - 1 ? "end" : "middle";
            const s = strong[i];
            const steps = s > 0.66 ? 3 : s > 0.33 ? 2 : 1;
            const bw = 7;
            const total = 3 * bw + 6;
            const bx = anchor === "start" ? x : anchor === "end" ? x - total : x - total / 2;
            const off = cur === i;
            return (
              <g key={ax.key}>
                <text
                  x={x}
                  y={y}
                  textAnchor={anchor}
                  dominantBaseline="middle"
                  className={["c-radar__label", ax.dim && "c-radar__dim", onAxis && "c-radar__click", off && "c-radar__label--off"].filter(Boolean).join(" ")}
                  style={ax.dim ? undefined : { fontSize: 11 + s * 3.5, fill: `rgba(243,244,246,${(0.35 + s * 0.65).toFixed(2)})` }}
                  onClick={onAxis && (() => onAxis(ax.key))}
                >
                  {ax.label}
                </text>
                {!ax.dim && (
                  <g className={`c-radar__imp${off ? " c-radar__imp--off" : ""}`}>
                    {[0, 1, 2].map((k) => (
                      <rect key={k} className={k < steps ? "c-radar__imp-on" : undefined} x={bx + k * (bw + 3)} y={y + 11} width={bw} height={3} rx={1.5} />
                    ))}
                  </g>
                )}
              </g>
            );
          })}
        </svg>
        {a && (
          <div
            ref={card}
            className={`c-radar__card${dock ? "" : " c-radar__card--below"}`}
            style={dock ? { left: `${(dock.x / W) * 100}%`, top: `${(dock.y / H) * 100}%` } : undefined}
          >
            <Card a={a} />
          </div>
        )}
      </div>
      <p className="c-radar__foot">Only habits where winners clearly differ from losers are drawn · more bars = matters more</p>
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
