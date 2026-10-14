import type { SessionTime } from "@shared/generated/types";

export interface WeekTotals {
  /** Monday 00:00, local time. */
  start: number;
  training: number;
  freeplay: number;
  ranked: number;
  /** Casual, private and other activities. */
  other: number;
}

/** Monday 00:00 (local time) of the week holding `ms`. */
export function weekStart(ms: number): number {
  const d = new Date(ms);
  d.setHours(0, 0, 0, 0);
  d.setDate(d.getDate() - ((d.getDay() + 6) % 7));
  return d.getTime();
}

function nextWeek(start: number): number {
  const d = new Date(start);
  d.setDate(d.getDate() + 7); // calendar days, so a clock change does not shift the week
  return d.getTime();
}

/** One entry per week from `from` to `to`, oldest first; weeks without play stay at zero. */
export function weekBuckets(times: SessionTime[], from: number, to: number): WeekTotals[] {
  const weeks: WeekTotals[] = [];
  for (let start = weekStart(from); start <= to; start = nextWeek(start)) {
    weeks.push({ start, training: 0, freeplay: 0, ranked: 0, other: 0 });
  }
  const byStart = new Map(weeks.map((w) => [w.start, w]));
  for (const t of times) {
    const week = byStart.get(weekStart(t.at));
    if (!week) continue;
    week.training += t.training_ms;
    week.freeplay += t.freeplay_ms;
    week.ranked += t.ranked_ms;
    week.other += t.other_ms;
  }
  return weeks;
}
