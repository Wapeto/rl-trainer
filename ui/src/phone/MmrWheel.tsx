import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { vibrate } from "@shared/vibrate";

export const STEP_PX = 14;
export const MMR_MIN = 0;
export const MMR_MAX = 3000;
const TAP_PX = 4;
const COAST_DECAY = 0.92;
/** The release speed comes from the moves of the last 100 ms, over at least one frame. */
const SAMPLE_MS = 100;
const MIN_WINDOW_MS = 16;
/** A finger that rested this long before lifting does not throw the wheel. */
const REST_MS = 80;
const FRAME_MS = 16;
const MIN_COAST_PX = 8;
const MAX_COAST_PX = 40;

interface Drag {
  pressed: boolean;
  /** This press stopped a coast: lifting it is not a tap that opens the number field. */
  caught: boolean;
  y: number;
  rest: number;
  moved: number;
  downAt: number;
  samples: { t: number; dy: number }[];
}

const idle = (): Drag => ({ pressed: false, caught: false, y: 0, rest: 0, moved: 0, downAt: 0, samples: [] });

/** Pixels per frame to coast at, from the last moves; 0 when the finger had stopped. */
function releaseSpeed(d: Drag, now: number): number {
  const last = d.samples[d.samples.length - 1];
  if (!last || now - last.t > REST_MS) return 0;
  const recent = d.samples.filter((s) => now - s.t <= SAMPLE_MS);
  const px = recent.reduce((sum, s) => sum + s.dy, 0);
  const windowMs = Math.max(MIN_WINDOW_MS, now - Math.max(d.downAt, now - SAMPLE_MS));
  const speed = (px / windowMs) * FRAME_MS;
  return Math.max(-MAX_COAST_PX, Math.min(MAX_COAST_PX, speed));
}

const clamp = (v: number) => Math.min(MMR_MAX, Math.max(MMR_MIN, v));

interface MmrWheelProps {
  label: string;
  value: number | null;
  last: number | null;
  onChange: (next: number | null) => void;
  buzz?: (ms: number) => void;
}

/** Drag down to raise, up to lower, tap to type (spec §6). */
export function MmrWheel({ label, value, last, onChange, buzz = vibrate }: MmrWheelProps) {
  const [typing, setTyping] = useState(value == null);
  const [text, setText] = useState(value?.toString() ?? "");
  const drag = useRef<Drag>(idle());
  const frame = useRef<number | null>(null);
  const current = useRef(value ?? 0);
  current.current = value ?? 0;

  function step(n: number) {
    if (n === 0) return;
    const next = clamp(current.current + n);
    if (next === current.current) return;
    for (let i = 0; i < Math.abs(next - current.current); i++) buzz(5);
    current.current = next;
    onChange(next);
  }

  function stopCoast() {
    if (frame.current != null) cancelAnimationFrame(frame.current);
    frame.current = null;
  }

  // Leaving the screen stops a coast still running.
  useEffect(() => stopCoast, []);

  function onDown(e: PointerEvent<HTMLDivElement>) {
    const caught = frame.current != null;
    stopCoast();
    e.currentTarget.setPointerCapture?.(e.pointerId);
    drag.current = { ...idle(), pressed: true, caught, y: e.clientY, downAt: performance.now() };
  }

  function onMove(e: PointerEvent<HTMLDivElement>) {
    const d = drag.current;
    if (!d.pressed) return;
    const now = performance.now();
    const dy = e.clientY - d.y;
    d.y = e.clientY;
    d.moved += Math.abs(dy);
    d.samples = [...d.samples.filter((s) => now - s.t <= SAMPLE_MS), { t: now, dy }];
    d.rest += dy;
    const steps = Math.trunc(d.rest / STEP_PX);
    d.rest -= steps * STEP_PX;
    step(steps);
  }

  function onUp() {
    const d = drag.current;
    if (!d.pressed) return;
    d.pressed = false;
    if (d.moved < TAP_PX) {
      if (!d.caught) {
        setText(String(current.current));
        setTyping(true);
      }
      return;
    }
    let speed = releaseSpeed(d, performance.now());
    if (Math.abs(speed) <= MIN_COAST_PX || typeof requestAnimationFrame !== "function") return;
    const coast = () => {
      frame.current = null;
      d.rest += speed;
      const steps = Math.trunc(d.rest / STEP_PX);
      d.rest -= steps * STEP_PX;
      step(steps);
      speed *= COAST_DECAY;
      if (Math.abs(speed) > 0.5) frame.current = requestAnimationFrame(coast);
    };
    frame.current = requestAnimationFrame(coast);
  }

  function onCancel() {
    drag.current.pressed = false;
  }

  function onKey(e: KeyboardEvent<HTMLDivElement>) {
    const by = { ArrowUp: 1, ArrowDown: -1, PageUp: 10, PageDown: -10 }[e.key];
    if (by === undefined) return;
    e.preventDefault();
    step(by);
  }

  function apply() {
    const raw = text.trim();
    const n = Number(raw);
    if (raw === "") {
      onChange(null);
      return;
    }
    if (Number.isInteger(n)) onChange(clamp(n));
    if (value != null || Number.isInteger(n)) setTyping(false);
  }

  if (typing) {
    return (
      <input
        className="wheel__input"
        inputMode="numeric"
        pattern="[0-9]*"
        aria-label={label}
        autoFocus={value != null}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onBlur={apply}
        onKeyDown={(e) => e.key === "Enter" && apply()}
      />
    );
  }
  const v = value ?? 0;
  const delta = last == null ? 0 : v - last;
  return (
    <div
      className="wheel"
      role="spinbutton"
      tabIndex={0}
      aria-label={label}
      aria-valuenow={v}
      aria-valuemin={MMR_MIN}
      aria-valuemax={MMR_MAX}
      onPointerDown={onDown}
      onPointerMove={onMove}
      onPointerUp={onUp}
      onPointerCancel={onCancel}
      onKeyDown={onKey}
    >
      {[2, 1].map((i) => (
        <span key={`a${i}`} className={`wheel__n wheel__n--${i}`} aria-hidden>
          {clamp(v + i)}
        </span>
      ))}
      <span className="wheel__now">{v}</span>
      {[1, 2].map((i) => (
        <span key={`b${i}`} className={`wheel__n wheel__n--${i}`} aria-hidden>
          {clamp(v - i)}
        </span>
      ))}
      {delta !== 0 && (
        <span className={`wheel__delta wheel__delta--${delta > 0 ? "up" : "down"}`}>
          {delta > 0 ? `+${delta}` : `−${-delta}`}
        </span>
      )}
    </div>
  );
}
