/** Pure geometry for the radar: its scale, which slice a point is in, and where the card docks. */

export const W = 700;
export const H = 530;
export const CX = W / 2;
export const CY = H / 2;
export const R = 110;
/** How far past the plot a pointer still picks a slice. */
export const PICK_REACH = 1.18;
/** Room the card keeps from the plot circle. */
const CLEAR = 12;

export interface Scale {
  lo: number;
  hi: number;
  zoomed: boolean;
}

/** Zoom only when it helps: every value (mine and the winners') sits in a narrow band away from the centre. */
export function radarScale(values: number[]): Scale {
  if (values.length === 0) return { lo: 0, hi: 100, zoomed: false };
  const lo = Math.floor((Math.min(...values) - 8) / 10) * 10;
  const hi = Math.min(100, Math.ceil((Math.max(...values) + 8) / 10) * 10);
  return lo >= 20 && hi - lo <= 70 ? { lo, hi, zoomed: true } : { lo: 0, hi: 100, zoomed: false };
}

/** Round values for the rings: every 10 when zoomed, else every 25; the outer edge is always one. */
export function ringValues({ lo, hi }: Scale): number[] {
  const step = hi - lo > 50 ? 25 : 10;
  const rings: number[] = [];
  for (let v = Math.ceil((lo + 1) / step) * step; v < hi; v += step) rings.push(v);
  return [...rings, hi];
}

/** Axis `i` of `n` points from the top, clockwise. */
export const angle = (i: number, n: number) => -Math.PI / 2 + (i * 2 * Math.PI) / n;

/** A point at `r` (0–1 of the radius, more for labels) on axis `i` of `n`. */
export function at(i: number, n: number, r: number): [number, number] {
  const a = angle(i, n);
  return [Math.round((CX + r * R * Math.cos(a)) * 10) / 10, Math.round((CY + r * R * Math.sin(a)) * 10) / 10];
}

/** Where a value (0–100) sits on axis `i` of `n` under `scale`; below the scale it stays at the centre. */
export function valuePoint(i: number, n: number, value: number, { lo, hi }: Scale): [number, number] {
  return at(i, n, Math.max(0, Math.min(1, (value - lo) / (hi - lo))));
}

/** The axis whose slice holds the point (dx, dy) from the centre, or null outside the picking reach. */
export function sliceAt(dx: number, dy: number, n: number): number | null {
  if (n === 0 || Math.hypot(dx, dy) > R * PICK_REACH) return null;
  let a = Math.atan2(dy, dx) + Math.PI / 2;
  if (a < 0) a += 2 * Math.PI;
  return Math.round(a / ((2 * Math.PI) / n)) % n;
}

/** How close a box comes to the centre. */
function distance(x: number, y: number, w: number, h: number): number {
  return Math.hypot(Math.max(x, Math.min(CX, x + w)) - CX, Math.max(y, Math.min(CY, y + h)) - CY);
}

/**
 * Where a w × h card for axis `i` of `n` goes: outward from the axis end, slid further out until it
 * clears the plot, inside the W × H box. Null when it cannot fit there (a narrow chart): the card then
 * goes under the chart.
 */
export function dockCard(i: number, n: number, w: number, h: number): { x: number; y: number } | null {
  const a = angle(i, n);
  const c = Math.cos(a);
  const s = Math.sin(a);
  const [ax, ay] = at(i, n, 1 + 14 / R);
  let x = c > 0.3 ? ax : c < -0.3 ? ax - w : ax - w / 2;
  let y = s > 0.3 ? ay : s < -0.3 ? ay - h : ay - h / 2;
  for (let k = 0; k < 100 && distance(x, y, w, h) < R + CLEAR; k++) {
    x += 2 * c;
    y += 2 * s;
  }
  x = Math.max(0, Math.min(W - w, x));
  y = Math.max(0, Math.min(H - h, y));
  return distance(x, y, w, h) >= R + CLEAR ? { x, y } : null;
}
