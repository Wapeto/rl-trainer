import type { ProgressRange } from "@shared/generated/types";

/** "+32", "−9", "0"; "—" when unknown. */
export function signed(n: number | null | undefined, digits = 0): string {
  if (n == null) return "—";
  const text = Math.abs(n).toFixed(digits);
  if (Number(text) === 0) return text;
  return n > 0 ? `+${text}` : `−${text}`;
}

/** A make-rate or win-rate change (0–1 fractions) in percentage points: "+7 pts". */
export function pointsChange(change: number | null | undefined): string {
  return change == null ? "—" : `${signed(change * 100)} pts`;
}

export function changeTone(n: number | null | undefined): "d-g" | "d-r" | "d-m" {
  if (n == null || n === 0) return "d-m";
  return n > 0 ? "d-g" : "d-r";
}

/** "8 Oct". */
export function shortDate(ms: number): string {
  return new Date(ms).toLocaleDateString("en-GB", { day: "numeric", month: "short" });
}

export function winPct(wins: number, losses: number): number | null {
  return wins + losses === 0 ? null : Math.round((wins * 100) / (wins + losses));
}

export const modeName = (teamSize: number) => `${teamSize}v${teamSize}`;

export const MODE_COLORS: Record<number, string> = { 1: "#7aa7ff", 2: "#6fe3a9", 3: "#c39bff" };

export const RANGE_LABEL: Record<ProgressRange, string> = { days30: "30 d", days90: "90 d", all: "All" };

export function emptyRangeText(range: ProgressRange): string {
  return range === "all" ? "Nothing recorded yet." : `Nothing played in the last ${range === "days30" ? 30 : 90} days.`;
}
