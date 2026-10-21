/** Pure geometry for the radar: its size for a width, where things sit, and which slice a point is in. */

/** How far past the plot a pointer still picks a slice. */
export const PICK_REACH = 1.18;
/** Room beside the plot for a label line, and above or below it for a label block with its bar. */
const LABEL_W = 124;
const LABEL_H = 64;
/** A label sits this far outside the plot. */
export const LABEL_GAP = 18;
const MIN_R = 70;
const MAX_R = 160;
/** Names longer than this wrap onto two lines. */
const WRAP = 16;

/** The plot's range on every spoke: 0 = the losers' level, 100 = the winners' level. */
export const T_MIN = -50;
export const T_MAX = 150;

/** The drawing's size in pixels, its centre and the plot radius. */
export interface Frame {
  w: number;
  h: number;
  r: number;
  cx: number;
  cy: number;
}

/** The largest plot that leaves room for the labels in `width` pixels, drawn 1:1 so text keeps its size. */
export function frame(width: number): Frame {
  const r = Math.round(Math.max(MIN_R, Math.min(MAX_R, width / 2 - LABEL_W - LABEL_GAP)));
  const h = 2 * (r + LABEL_H);
  return { w: width, h, r, cx: width / 2, cy: h / 2 };
}

/** How far out (0–1 of the radius) a position sits; the centre keeps a hole so −50 stays visible. */
export function radius(t: number): number {
  return 0.15 + ((Math.max(T_MIN, Math.min(T_MAX, t)) - T_MIN) / (T_MAX - T_MIN)) * 0.85;
}

/** Axis `i` of `n` points from the top, clockwise. */
export const angle = (i: number, n: number) => -Math.PI / 2 + (i * 2 * Math.PI) / n;

/** A point at `r` (0–1 of the radius) on axis `i` of `n`, plus `px` more pixels outward. */
export function at(f: Frame, i: number, n: number, r: number, px = 0): [number, number] {
  const a = angle(i, n);
  const d = r * f.r + px;
  return [Math.round((f.cx + d * Math.cos(a)) * 10) / 10, Math.round((f.cy + d * Math.sin(a)) * 10) / 10];
}

/** Where position `t` sits on axis `i` of `n`. */
export const valuePoint = (f: Frame, i: number, n: number, t: number): [number, number] => at(f, i, n, radius(t));

/** The axis whose slice holds the point (dx, dy) from the centre, or null outside the picking reach. */
export function sliceAt(dx: number, dy: number, n: number, r: number): number | null {
  if (n === 0 || Math.hypot(dx, dy) > r * PICK_REACH) return null;
  let a = Math.atan2(dy, dx) + Math.PI / 2;
  if (a < 0) a += 2 * Math.PI;
  return Math.round(a / ((2 * Math.PI) / n)) % n;
}

/** A name as one line, or two split at the space nearest its middle. */
export function labelLines(label: string): string[] {
  if (label.length <= WRAP) return [label];
  const spaces = [...label].flatMap((c, i) => (c === " " ? [i] : []));
  if (spaces.length === 0) return [label];
  const cut = spaces.reduce((b, i) => (Math.abs(i - label.length / 2) < Math.abs(b - label.length / 2) ? i : b));
  return [label.slice(0, cut), label.slice(cut + 1)];
}
