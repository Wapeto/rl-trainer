import type { ModeStat } from "@shared/generated/types";

export type StatGroup = "scoreboard" | "shooting" | "touches" | "movement" | "boost" | "team";

export const GROUPS: { id: StatGroup; label: string }[] = [
  { id: "scoreboard", label: "Scoreboard" },
  { id: "shooting", label: "Shooting" },
  { id: "touches", label: "Touches" },
  { id: "movement", label: "Movement" },
  { id: "boost", label: "Boost" },
  { id: "team", label: "Demos & team" },
];

export interface StatMeta {
  label: string;
  /** The tab it is shown in; win rate sits above the tabs. */
  group: StatGroup | null;
  unit: "%" | "km/h" | "";
  perGame?: boolean;
  /** Estimated from 10 samples a second. */
  estimated?: boolean;
  decimals: number;
}

const pct = (label: string, group: StatGroup | null): StatMeta => ({ label, group, unit: "%", decimals: 0 });
const game = (label: string, group: StatGroup, decimals = 1): StatMeta => ({ label, group, unit: "", perGame: true, decimals });

export const STATS: Record<ModeStat, StatMeta> = {
  win_rate: pct("Win rate", null),
  goals: game("Goals", "scoreboard"),
  assists: game("Assists", "scoreboard"),
  saves: game("Saves", "scoreboard"),
  shots: game("Shots", "scoreboard"),
  score: game("Score", "scoreboard", 0),
  shooting_pct: pct("Shooting %", "shooting"),
  goal_speed: { label: "Goal speed", group: "shooting", unit: "km/h", decimals: 0 },
  epic_saves: game("Epic saves", "shooting"),
  touches: game("Touches", "touches", 0),
  aerial_touches: game("Aerial touches", "touches"),
  hit_power: { label: "Hit power", group: "touches", unit: "km/h", decimals: 0 },
  hard_hits: game("Hard hits", "touches"),
  defensive_third: pct("Touches in defensive third", "touches"),
  attacking_third: pct("Touches in attacking third", "touches"),
  airborne: pct("Airborne", "movement"),
  on_wall: pct("On wall", "movement"),
  avg_speed: { label: "Average speed", group: "movement", unit: "km/h", decimals: 0 },
  supersonic: pct("Supersonic", "movement"),
  avg_boost: { label: "Average boost", group: "boost", unit: "", decimals: 0 },
  zero_boost: pct("At 0 boost", "boost"),
  full_boost: pct("Full boost", "boost"),
  boosting: pct("Boosting", "boost"),
  boost_per_minute: { label: "Boost used per minute", group: "boost", unit: "", decimals: 0, estimated: true },
  big_pads: { ...game("Big pads", "boost"), estimated: true },
  demos_given: game("Demos given", "team"),
  demos_taken: game("Demos taken", "team"),
  share_score: pct("Share of team score", "team"),
  share_goals: pct("Share of team goals", "team"),
  share_saves: pct("Share of team saves", "team"),
  possession: pct("Possession", "team"),
};

export function formatStat(stat: ModeStat, value: number | null | undefined): string {
  if (value == null) return "—";
  const m = STATS[stat];
  const n = value.toFixed(m.decimals);
  if (m.unit === "%") return `${n}%`;
  return m.unit ? `${n} ${m.unit}` : n;
}
