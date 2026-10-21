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

/** The plot's range on every spoke: 0 = the losers' level, 100 = the winners' level. */
export const T_MIN = -50;
export const T_MAX = 150;

/** How far out (0–1 of the radius) a position sits; the centre keeps a hole so −50 stays visible. */
export function radius(t: number): number {
  return 0.15 + ((Math.max(T_MIN, Math.min(T_MAX, t)) - T_MIN) / (T_MAX - T_MIN)) * 0.85;
}

/** Axis `i` of `n` points from the top, clockwise. */
export const angle = (i: number, n: number) => -Math.PI / 2 + (i * 2 * Math.PI) / n;

/** A point at `r` (0–1 of the radius, more for labels) on axis `i` of `n`. */
export function at(i: number, n: number, r: number): [number, number] {
  const a = angle(i, n);
  return [Math.round((CX + r * R * Math.cos(a)) * 10) / 10, Math.round((CY + r * R * Math.sin(a)) * 10) / 10];
}

/** Where position `t` sits on axis `i` of `n`. */
export const valuePoint = (i: number, n: number, t: number): [number, number] => at(i, n, radius(t));

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
