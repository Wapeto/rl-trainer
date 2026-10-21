import { useId, useLayoutEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import "./charts.css";
import { CX, CY, H, R, W, at, dockCard, radarScale, ringValues, sliceAt, valuePoint } from "./radarGeometry";

/** One axis of a radar: ranks 0–100, outward is better. */
export interface RadarAxis {
  key: string;
  label: string;
  me: number | null;
  winners: number;
  /** Read by screen readers; on a dimmed axis, the card says it too. */
  tip?: string;
  /** Drawn faint: nothing in it separates winners from losers. */
  dim?: boolean;
  /** The real values for the card, already formatted, and their unit ("per 5 min"). */
  you?: string;
  winnersValue?: string;
  unit?: string;
}

const rank = (n: number) => Math.round(n);

/** "Biggest gap: X · Ahead on Y, Z" over the axes that count. */
export function takeaway(axes: RadarAxis[]): string | null {
  const real = axes.filter((a): a is RadarAxis & { me: number } => !a.dim && a.me != null);
  if (real.length === 0) return null;
  const worst = [...real].sort((a, b) => a.me - a.winners - (b.me - b.winners))[0];
  const ahead = real.filter((a) => rank(a.me) > rank(a.winners)).map((a) => a.label);
  const gap = rank(worst.me) < rank(worst.winners) ? `Biggest gap: ${worst.label}` : "At or above the winners everywhere";
  return ahead.length > 0 && gap.startsWith("Biggest") ? `${gap} · Ahead on ${ahead.join(", ")}` : gap;
}

function verdict(a: RadarAxis): { text: string; good: boolean | null } {
  if (a.dim) return { text: a.tip ?? "No clear difference between winners and losers", good: null };
  if (a.me == null) return { text: "Not measured for you", good: null };
  return rank(a.me) >= rank(a.winners)
    ? { text: "At or above the winners", good: true }
    : { text: `Winners rank ${rank(a.winners)}, you rank ${rank(a.me)}`, good: false };
}

function Card({ a }: { a: RadarAxis }) {
  const v = verdict(a);
  const row = (who: string, value: number | null, shown: string | undefined, cls: string) => (
    <div className="c-radar__line">
      <span>{who}</span>
      <span className="c-radar__bar">
        <i className={cls} style={{ width: `${value == null ? 0 : Math.max(2, Math.min(100, value))}%` }} />
      </span>
      <b>{shown ?? (value == null ? "—" : `rank ${rank(value)}`)}</b>
    </div>
  );
  return (
    <>
      <div className="c-radar__name">
        {a.label}
        {a.unit && <span>{a.unit}</span>}
      </div>
      {!a.dim && row("You", a.me, a.you, "c-radar__b-me")}
      {!a.dim && row("Winners", a.winners, a.winnersValue, "c-radar__b-win")}
      <div className={`c-radar__verdict${v.good === true ? " c-radar__good" : v.good === false ? " c-radar__bad" : ""}`}>{v.text}</div>
    </>
  );
}

const cardText = (a: RadarAxis) =>
  a.dim ? `${a.label}: ${verdict(a).text}` : `${a.label}: you ${a.you ?? (a.me == null ? "not measured" : `rank ${rank(a.me)}`)}, winners ${a.winnersValue ?? `rank ${rank(a.winners)}`}. ${verdict(a).text}.`;

/**
 * You against the winners on rank axes (spec §5.2): slice hover, a card docked outside the plot,
 * automatic zoom, keyboard and tap.
 */
export function Radar({ label, axes, onAxis }: { label: string; axes: RadarAxis[]; onAxis?: (key: string) => void }) {
  const n = axes.length;
  const id = useId();
  const plot = useRef<HTMLDivElement>(null);
  const card = useRef<HTMLDivElement>(null);
  const [cur, setCur] = useState<number | null>(null);
  const [dock, setDock] = useState<{ x: number; y: number; w: number; h: number } | null>(null);
  const scale = radarScale(axes.filter((a) => !a.dim).flatMap((a) => (a.me == null ? [a.winners] : [a.me, a.winners])));
  const pt = (i: number, v: number) => valuePoint(i, n, v, scale);
  const poly = (vs: number[]) => vs.map((v, i) => pt(i, v).join(",")).join(" ");
  const take = takeaway(axes);

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

  return (
    <figure className="c-radar" aria-label={label}>
      <div className="c-radar__hd">
        <div>
          {take && <p className={`c-radar__take${take.startsWith("Biggest") ? "" : " c-radar__good"}`}>{take}</p>}
          <p className="c-radar__how">How you rank among all players of those games. Further out is better.</p>
        </div>
        <div className="c-radar__key" aria-hidden="true">
          <span>
            <i className="c-radar__k-me" /> You
          </span>
          <span>
            <i className="c-radar__k-winners" /> Winners
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
              <stop offset="0" stopColor="#7aa7ff" stopOpacity={0.04} />
              <stop offset="1" stopColor="#7aa7ff" stopOpacity={0.34} />
            </radialGradient>
          </defs>
          <polygon className="c-radar__disc" points={axes.map((_, i) => at(i, n, 1).join(",")).join(" ")} />
          {ringValues(scale).map((v) => (
            <polygon key={v} className={v === scale.hi ? "c-radar__edge" : "c-radar__ring"} points={poly(axes.map(() => v))} />
          ))}
          {ringValues(scale)
            .filter((v) => v < scale.hi)
            .map((v) => {
              const [x, y] = pt(0, v);
              return (
                <text key={v} className="c-radar__tick" x={x + 6} y={y + 3.5}>
                  {v}
                </text>
              );
            })}
          {axes.map((ax, i) => {
            const [x, y] = at(i, n, 1);
            return <line key={ax.key} className={`c-radar__spoke${cur === i ? " c-radar__spoke--on" : ""}`} x1={CX} y1={CY} x2={x} y2={y} />;
          })}
          <g className="c-radar__shapes">
            <polygon className="c-radar__winners" points={poly(axes.map((ax) => ax.winners))} />
            <polygon className="c-radar__me" fill={`url(#${id}me)`} points={poly(axes.map((ax) => ax.me ?? 0))} />
            {axes.map((ax, i) => {
              const [x, y] = pt(i, ax.winners);
              return <circle key={ax.key} className="c-radar__dot-win" cx={x} cy={y} r={cur === i ? 5 : 2.75} />;
            })}
            {axes.map((ax, i) => {
              if (ax.me == null) return null;
              const [x, y] = pt(i, ax.me);
              return <circle key={ax.key} className="c-radar__dot-me" cx={x} cy={y} r={cur === i ? 6.5 : 3.75} />;
            })}
          </g>
          {leader && <polyline className="c-radar__leader" points={leader} />}
          {axes.map((ax, i) => {
            const [x, y] = at(i, n, 1 + 22 / R);
            const anchor = x > CX + 1 ? "start" : x < CX - 1 ? "end" : "middle";
            return (
              <text
                key={ax.key}
                x={x}
                y={y}
                textAnchor={anchor}
                dominantBaseline="middle"
                className={["c-radar__label", ax.dim && "c-radar__dim", onAxis && "c-radar__click", cur === i && "c-radar__label--off"].filter(Boolean).join(" ")}
                onClick={onAxis && (() => onAxis(ax.key))}
              >
                {ax.label}
              </text>
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
      <p className="c-radar__scale">{scale.zoomed ? `Rank scale ${scale.lo}–${scale.hi}, zoomed in so the gaps are visible` : "Rank scale 0–100"}</p>
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
