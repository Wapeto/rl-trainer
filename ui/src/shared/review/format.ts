import type { ReplayToSave, ReviewItem, SessionReview, TrainingItem } from "@shared/generated/types";

export const modeName = (teamSize: number) => `${teamSize}v${teamSize}`;

/** A mode is compared once it has this many earlier ranked matches (`MIN_EARLIER` in rl-core). */
export const MIN_EARLIER = 5;

export function formatItemValue(item: ReviewItem, v: number): string {
  switch (item.unit) {
    case "percent":
      return `${Math.round(v)}%`;
    case "kmh":
      return `${Math.round(v)} km/h`;
    case "per_minute":
      return `${Math.round(v)}/min`;
    case "boost":
      return `${Math.round(v)}`;
    default:
      return (Math.round(v * 10) / 10).toFixed(1).replace(/\.0$/, "");
  }
}

interface Delta {
  up: boolean;
  short: string;
  long: string;
}

/** Points for percentages, a relative change otherwise, the plain difference when the usual is under 1. */
function delta(item: ReviewItem): Delta {
  // From the numbers the row shows, so "49%, usual 31%" reads +18, not the unrounded +19.
  const shown = (v: number) => Number(formatItemValue(item, v).replace(/[^0-9.-]/g, ""));
  const value = shown(item.value);
  const usual = shown(item.usual);
  const diff = value - usual;
  const up = diff >= 0;
  const size = Math.abs(diff);
  if (item.unit === "percent") {
    const n = Math.round(size);
    return { up, short: `${n} pts`, long: `${n} ${n === 1 ? "point" : "points"}` };
  }
  if (Math.abs(usual) < 1) {
    const v = formatItemValue(item, size);
    return { up, short: v, long: v };
  }
  const p = `${Math.round((size / Math.abs(usual)) * 100)}%`;
  return { up, short: p, long: p };
}

export function formatDelta(item: ReviewItem): string {
  const d = delta(item);
  return `${d.up ? "▲" : "▼"} ${d.up ? "+" : "−"}${d.short}`;
}

/** The row as a sentence for screen readers ("Touches 98, usual 79, 24% above"). */
export function describeItem(item: ReviewItem): string {
  const d = delta(item);
  const v = formatItemValue(item, item.value);
  const u = formatItemValue(item, item.usual);
  return `${item.label} ${v}, usual ${u}, ${d.long} ${d.up ? "above" : "below"}`;
}

/** Replay stats that cover only part of the session ("2 of 3 matches"). */
export function coverageNote(item: ReviewItem): string | null {
  return item.covered < item.of ? `${item.covered} of ${item.of} matches` : null;
}

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** "3 things stood out · 4 replays to save", or what stands in for either half. */
export function summaryLine(r: SessionReview): string {
  const items = r.modes.reduce((sum, m) => sum + m.items.length, 0);
  const parts: string[] = [];
  if (items > 0) parts.push(`${plural(items, "thing", "things")} stood out`);
  else if (r.modes.some((m) => m.earlier >= MIN_EARLIER)) parts.push("Nothing stood out against your usual");
  else if (r.modes.length > 0) parts.push("Nothing to compare yet");
  if (r.to_save.length > 0) parts.push(`${plural(r.to_save.length, "replay", "replays")} to save`);
  return parts.join(" · ");
}

export function trainingRate(t: TrainingItem): string | null {
  return t.make_rate == null ? null : `${Math.round(t.make_rate * 100)}%`;
}

export function trainingBelow(t: TrainingItem): boolean {
  return t.make_rate != null && t.make_rate < t.target_rate;
}

function minutesAfter(start: number, at: number): number {
  return Math.max(0, Math.round((at - start) / 60_000));
}

export function trainingDetail(t: TrainingItem): string {
  const parts: string[] = [`target ${Math.round(t.target_rate * 100)}%`];
  if (t.best_makes != null && t.best_at != null) {
    const m = minutesAfter(t.started_at, t.best_at);
    parts.push(`Best stretch ${t.best_makes}/20 ${m < 5 ? "at the start" : `after ${m} min`}`);
  }
  if (t.dropped_at != null) parts.push(`dropped after ${minutesAfter(t.started_at, t.dropped_at)} min`);
  if (t.quick_resets > 0) parts.push(`${t.quick_resets} quick resets left out`);
  return parts.join(" · ");
}

export interface ToSaveParts {
  result: string;
  tone: "win" | "loss" | "other";
  mode: string;
  time: string;
}

export function toSaveParts(m: ReplayToSave): ToSaveParts {
  const tone = m.result === "win" ? "win" : m.result === "loss" ? "loss" : "other";
  const letter = tone === "win" ? "W" : tone === "loss" ? "L" : "–";
  const time = new Date(m.started_at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  return {
    result: `${letter} ${m.team_score}–${m.opponent_score}`,
    tone,
    mode: m.team_size ? modeName(m.team_size) : "Match",
    time,
  };
}
