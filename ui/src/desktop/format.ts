import { longDuration, timeOfDay } from "@shared/format";

/** "Thursday 8 October". */
export function longDay(ms: number): string {
  return new Date(ms).toLocaleDateString("en-GB", { weekday: "long", day: "numeric", month: "long" });
}

/** "20:14 – 21:52", or "20:14 – now" while it runs. */
export function span(start: number, end: number | null): string {
  return `${timeOfDay(start)} – ${end == null ? "now" : timeOfDay(end)}`;
}

/** "1 h 38"-style duration, or null while it runs. */
export function duration(start: number, end: number | null): string | null {
  return end == null ? null : longDuration(end - start);
}

export function plural(n: number, word: string, words = `${word}s`): string {
  return `${n} ${n === 1 ? word : words}`;
}

/** "58%", or "—". */
export function percent(rate: number | null | undefined): string {
  return rate == null ? "—" : `${Math.round(rate * 100)}%`;
}
