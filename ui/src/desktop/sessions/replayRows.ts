import type { PlayerReplayStats, ReplayStats } from "@shared/generated/types";
import { type Gloss, REPLAY_STATS } from "@shared/glossary";

export type Better = "high" | "low" | "none";
/** How a stored value is shown: as is, per 5 minutes, as a share of the match, or of touches. */
export type Kind = "raw" | "per5" | "share" | "ofTouches";

export interface StatRow {
  key: keyof PlayerReplayStats;
  label: string;
  gloss: Gloss;
  kind: Kind;
  format: (n: number) => string;
  better: Better;
  /** Only shown in 2v2 and 3v3. */
  teamOnly?: boolean;
}

export interface StatGroup {
  name: string;
  teamOnly?: boolean;
  rows: StatRow[];
}

/** Pool matches before the winners' benchmark exists (rl-core coach::benchmark::MIN_POOL_MATCHES). */
export const MIN_POOL_MATCHES = 10;
const FIVE_MIN = 300;

const g = (title: string, text: string): Gloss => ({ title, text });
const count = (n: number) => String(Math.round(n));
const one = (n: number) => n.toFixed(1);
const pct = (n: number) => `${Math.round(n)}%`;
type RowOpts = { kind?: Kind; format?: (n: number) => string; better?: Better; teamOnly?: boolean; gloss?: Gloss };
const row = (key: keyof PlayerReplayStats, label: string, text: string, o: RowOpts = {}): StatRow => {
  const kind = o.kind ?? "per5";
  const format = o.format ?? (kind === "per5" ? one : kind === "raw" ? count : pct);
  return { key, label, gloss: o.gloss ?? g(label, text), kind, format, better: o.better ?? "high", teamOnly: o.teamOnly };
};

/** The value shown for a row; the winners' and losers' means are pre-scaled to 5 minutes, so the same maths fits them. */
export function statValue(r: StatRow, s: PlayerReplayStats): number | null {
  const v = s[r.key];
  if (v == null) return null;
  const t = s.tracked_seconds;
  if (r.kind === "raw") return v;
  if (r.kind === "ofTouches") return s.touches ? (v * 100) / s.touches : null;
  if (!t) return null;
  return r.kind === "per5" ? (v * FIVE_MIN) / t : (v * 100) / t;
}

/** The stats the insights use, first and open (spec §4.1). */
export const KEY_STATS: StatGroup = {
  name: "Key stats",
  rows: [
    row("pct_zero_boost", "Time at 0 boost", "Share of the match spent with no boost.", { kind: "raw", format: pct, better: "low" }),
    row("boost_used_supersonic", "Boost at full speed", "Boost spent while already supersonic, per 5 minutes: it adds no speed.", { better: "low" }),
    row("touches_low_boost", "Touches under 20 boost", "", { kind: "ofTouches", better: "low", gloss: REPLAY_STATS.low_boost_touches }),
    row("fifty_wins", "50/50s won", REPLAY_STATS.fifties.text),
    row("fifty_losses", "50/50s lost", REPLAY_STATS.fifties.text, { better: "low" }),
    row("pct_in_front_of_ball", "In front of the ball", "Share of the match spent between the ball and the other goal.", { kind: "raw", format: pct, better: "low" }),
    row("secs_first_man", "1st man", "Share of the match you were the teammate closest to the ball.", { kind: "share", better: "none", teamOnly: true }),
    row("kickoff_wins", "Kickoffs won", REPLAY_STATS.kickoffs.text, { kind: "raw" }),
    row("kickoff_losses", "Kickoffs lost", REPLAY_STATS.kickoffs.text, { kind: "raw", better: "low" }),
    row("demos_taken", "Demos taken", "Times you were demolished, per 5 minutes.", { better: "low" }),
    row("whiffs", "Whiffs", "", { better: "low", gloss: REPLAY_STATS.whiffs }),
    row("boost_stolen", "Boost stolen", "Boost picked up in the other team's half, per 5 minutes."),
  ],
};

/** The folded tables, in order (replay stats page spec §3). */
export const STAT_GROUPS: StatGroup[] = [
  {
    name: "Core",
    rows: [
      row("score", "Score", "Points on the scoreboard.", { kind: "raw" }),
      row("goals", "Goals", "Goals scored.", { kind: "raw" }),
      row("assists", "Assists", "Passes that led to a goal.", { kind: "raw" }),
      row("saves", "Saves", "Shots stopped.", { kind: "raw" }),
      row("shots", "Shots", "Shots on goal.", { kind: "raw" }),
    ],
  },
  {
    name: "Positioning",
    rows: [
      row("pct_behind_ball", "Behind the ball", "", { kind: "raw", format: pct, gloss: REPLAY_STATS.behind_ball }),
      row("pct_in_front_of_ball", "In front of the ball", "Share of the match spent between the ball and the other goal.", { kind: "raw", format: pct, better: "low" }),
      row("pct_defensive_third", "Defensive third", "Share of the match spent in your own third of the field.", { kind: "raw", format: pct, better: "none" }),
      row("pct_neutral_third", "Middle third", "Share of the match spent in the middle third.", { kind: "raw", format: pct, better: "none" }),
      row("pct_offensive_third", "Offensive third", "Share of the match spent in the other team's third.", { kind: "raw", format: pct, better: "none" }),
      row("pct_shadow_defense", "Shadow defence", "Share of the match spent retreating between an attacker with the ball and your goal.", { kind: "raw", format: pct }),
      row("avg_distance_to_ball_m", "Distance to ball", "Average distance from the ball, in metres.", { kind: "raw", format: (n) => `${n.toFixed(1)} m`, better: "none" }),
    ],
  },
  {
    name: "Rotation",
    teamOnly: true,
    rows: [
      row("secs_first_man", "1st man", "Share of the match you were the teammate closest to the ball.", { kind: "share", better: "none" }),
      row("secs_second_man", "2nd man", "Share of the match you were the next closest.", { kind: "share", better: "none" }),
      row("secs_third_man", "3rd man", "Share of the match you were the farthest back (3v3).", { kind: "share", better: "none" }),
      row("became_first_man", "Became 1st man", "Times you rotated into the first-man spot, per 5 minutes.", { better: "none" }),
      row("pct_most_back", "Last back", "Share of the match you were the player of your team nearest your own goal.", { kind: "raw", format: pct, better: "none" }),
    ],
  },
  {
    name: "Boost",
    rows: [
      row("boost_collected", "Collected", "Boost picked up per 5 minutes, on the 0–100 scale (a big pad is 100)."),
      row("boost_used", "Used", "Boost spent per 5 minutes.", { better: "none" }),
      row("boost_stolen", "Stolen", "Boost picked up in the other team's half, per 5 minutes."),
      row("big_pads", "Big pads", "", { gloss: REPLAY_STATS.big_pads }),
      row("small_pads", "Small pads", "Small boost pads picked up, per 5 minutes."),
      row("pct_zero_boost", "Time at 0", "Share of the match spent with no boost.", { kind: "raw", format: pct, better: "low" }),
      row("pct_full_boost", "Time at 100", "Share of the match spent with full boost.", { kind: "raw", format: pct, better: "none" }),
      row("boost_used_supersonic", "Used at full speed", "Boost spent while already supersonic: it adds no speed.", { better: "low" }),
    ],
  },
  {
    name: "Touches",
    rows: [
      row("touches", "Touches", "Times the player hit the ball, per 5 minutes."),
      row("hard_hits", "Hard hits", "Touches that sent the ball off fast."),
      row("aerial_touches", "Aerial", "Touches in the air."),
      row("control_touches", "Control", "Soft touches that kept the ball close."),
      row("first_touches", "First touches", "Touches that took the ball from the other team or a loose ball."),
    ],
  },
  {
    name: "Challenges",
    rows: [
      row("fifty_neutral", "50/50s even", "Open-play 50/50s neither player won.", { better: "none" }),
      row("beaten_to_ball", "Beaten to the ball", "Times an opponent reached the ball first when you went for it.", { better: "low" }),
    ],
  },
  {
    name: "Mechanics",
    rows: [
      row("flicks", "Flicks", "Flicks off a dribble."),
      row("wavedashes", "Wavedashes", "Wavedashes on landing."),
      row("half_flips", "Half-flips", "Half-flips to turn around."),
      row("speed_flips", "Speed flips", "Speed flips, mostly on kickoffs."),
    ],
  },
  {
    name: "Movement",
    rows: [
      row("avg_speed_kmh", "Average speed", "Average car speed.", { kind: "raw", format: (n) => `${Math.round(n)} km/h` }),
      row("pct_supersonic", "Supersonic", "Share of the match at full speed.", { kind: "raw", format: pct }),
      row("pct_on_ground", "On the ground", "Share of the match on the ground.", { kind: "raw", format: pct, better: "none" }),
      row("pct_high_air", "High in the air", "Share of the match high in the air.", { kind: "raw", format: pct, better: "none" }),
    ],
  },
  {
    name: "Physical",
    rows: [
      row("demos_inflicted", "Demos given", "Opponents demolished, per 5 minutes."),
      row("bumps_inflicted", "Bumps given", "Opponents bumped, per 5 minutes."),
      row("bumps_taken", "Bumps taken", "Times you were bumped, per 5 minutes.", { better: "low" }),
    ],
  },
  {
    name: "Passing",
    teamOnly: true,
    rows: [
      row("passes_completed", "Passes", "Touches a teammate followed up, per 5 minutes."),
      row("passes_received", "Passes received", "Teammate touches you followed up, per 5 minutes."),
    ],
  },
];

/** The value to bold in a row: none when fewer than two are known or all are equal. */
export function bestValue(values: (number | null)[], better: Better): number | null {
  const known = values.filter((v): v is number => v != null);
  if (better === "none" || known.length < 2 || known.every((v) => v === known[0])) return null;
  return better === "high" ? Math.max(...known) : Math.min(...known);
}

/** My older coach stats in the page's shape, for matches whose players were not read. */
export function fromMine(r: ReplayStats): PlayerReplayStats {
  return {
    tracked_seconds: r.tracked_seconds,
    pct_behind_ball: r.pct_behind_ball,
    pct_in_front_of_ball: r.pct_in_front_of_ball,
    pct_defensive_third: r.pct_defensive_third,
    pct_neutral_third: r.pct_neutral_third,
    pct_offensive_third: r.pct_offensive_third,
    secs_first_man: r.secs_first_man,
    secs_second_man: r.secs_second_man,
    secs_third_man: r.secs_third_man,
    became_first_man: r.became_first_man,
    big_pads: r.big_pads,
    small_pads: r.small_pads,
    touches: r.touches,
    touches_low_boost: r.touches_low_boost,
    fifty_wins: r.fifty_wins,
    fifty_losses: r.fifty_losses,
    kickoff_wins: r.kickoff_wins,
    kickoff_losses: r.kickoff_losses,
    whiffs: r.whiffs,
  } as PlayerReplayStats;
}
