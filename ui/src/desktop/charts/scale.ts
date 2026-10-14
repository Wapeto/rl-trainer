/** Maps [d0, d1] onto [r0, r1]; a single value (d0 = d1) maps to the middle. */
export function scale(d0: number, d1: number, r0: number, r1: number): (v: number) => number {
  if (d1 === d0) return () => (r0 + r1) / 2;
  return (v) => r0 + ((v - d0) / (d1 - d0)) * (r1 - r0);
}

/** The value range to draw: padded by 10%, a flat line by 1; never below 0 when no value is. */
export function domain(values: number[]): [number, number] {
  if (values.length === 0) return [0, 1];
  const lo = Math.min(...values);
  const hi = Math.max(...values);
  const pad = (hi - lo) * 0.1 || 1;
  return [lo >= 0 ? Math.max(0, lo - pad) : lo - pad, hi + pad];
}

/** The first day of each month after t0 up to t1, local time. */
export function monthTicks(t0: number, t1: number): number[] {
  const ticks: number[] = [];
  const d = new Date(t0);
  d.setHours(0, 0, 0, 0);
  d.setDate(1);
  d.setMonth(d.getMonth() + 1);
  while (d.getTime() <= t1) {
    ticks.push(d.getTime());
    d.setMonth(d.getMonth() + 1);
  }
  return ticks;
}

export interface AxisDate {
  at: number;
  label: string;
  anchor: "start" | "middle" | "end";
}

const dayLabel = (t: number) => new Date(t).toLocaleDateString("en-GB", { day: "numeric", month: "short" });
const monthLabel = (t: number) => new Date(t).toLocaleDateString("en-GB", { month: "short" });

/** Dates under a chart: each month over a span holding two or more month starts, otherwise the
 * first and last day. */
export function axisDates(t0: number, t1: number): AxisDate[] {
  const months = monthTicks(t0, t1);
  if (months.length >= 2) return months.map((at) => ({ at, label: monthLabel(at), anchor: "middle" }));
  if (dayLabel(t0) === dayLabel(t1)) return [{ at: t0, label: dayLabel(t0), anchor: "middle" }];
  return [
    { at: t0, label: dayLabel(t0), anchor: "start" },
    { at: t1, label: dayLabel(t1), anchor: "end" },
  ];
}

export function axisValue(v: number, unit: string): string {
  const n = Math.abs(v) < 10 ? v.toFixed(1) : String(Math.round(v));
  return unit === "%" ? `${n}%` : n;
}
