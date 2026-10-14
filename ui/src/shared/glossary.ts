import type { ModeStat } from "./generated/types";
import { HISTORY_SIZE } from "./review/history";

/** A hint: a bold title over one or two plain sentences. */
export interface Gloss {
  title: string;
  text: string;
}

const g = (title: string, text: string): Gloss => ({ title, text });

/** Live-feed stats, titled like the coach labels them (`mode_stat_meta` in rl-core). */
export const MODE_STATS: Record<ModeStat, Gloss> = {
  win_rate: g("Win rate", "Share of your matches you won."),
  goals: g("Goals", "Goals you scored, per match."),
  assists: g("Assists", "Passes that led to a teammate's goal, per match."),
  saves: g("Saves", "Shots on your goal you stopped, per match."),
  shots: g("Shots", "Hits the game counted as shots on goal, per match."),
  score: g("Score", "Your in-game points, per match."),
  shooting_pct: g("Shooting", "Your goals divided by your shots."),
  goal_speed: g("Goal speed", "How fast the ball was going on your goals, on average."),
  epic_saves: g("Epic saves", "Saves the game rated as epic, per match."),
  touches: g("Touches", "How many times you hit the ball, per match."),
  aerial_touches: g("Aerial touches", "Touches with the ball higher than about one car jump, per match."),
  hit_power: g("Hit power", "How fast the ball left your car after your touches, on average."),
  hard_hits: g("Hard hits", "Touches that sent the ball faster than the hard-hit speed in Settings (80 km/h by default), per match."),
  defensive_third: g("Touches in defence", "Share of your touches made in your own third of the field."),
  attacking_third: g("Touches in attack", "Share of your touches made in the opponents' third of the field."),
  airborne: g("Airborne", "Share of play time your car was in the air."),
  on_wall: g("On the wall", "Share of play time your car was driving on a wall."),
  avg_speed: g("Average speed", "Your car's average speed over play time."),
  supersonic: g("Supersonic", "Share of the match you drove at top speed (when the trail shows behind your car)."),
  avg_boost: g("Average boost", "The boost you held on average, from 0 to 100."),
  zero_boost: g("Time at 0 boost", "Share of the match you had no boost at all."),
  full_boost: g("Time at full boost", "Share of play time you held 100 boost."),
  boosting: g("Time boosting", "Share of play time you were using boost."),
  boost_per_minute: g("Boost used", "Boost you used per minute of play. Estimated from 10 readings a second."),
  big_pads: g("Big pads", "Full boost pads you picked up, per match. Estimated from jumps in your boost."),
  demos_given: g("Demos", "Opponents you demolished, per match."),
  demos_taken: g("Demoed", "Times you were demolished, per match."),
  share_score: g("Share of team score", "Your part of your team's total score."),
  share_goals: g("Share of team goals", "Your part of your team's goals."),
  share_saves: g("Share of team saves", "Your part of your team's saves."),
  possession: g("Possession", "Share of play time your team touched the ball last."),
};

/** Replay stats: the five the coach compares, and the ones only the match tiles show. */
export type ReplayKey =
  | "low_boost_touches"
  | "behind_ball"
  | "first_man_share"
  | "fifty_wins"
  | "whiffs"
  | "big_pads"
  | "fifties"
  | "kickoffs"
  | "first_second_man";

export const REPLAY_STATS: Record<ReplayKey, Gloss> = {
  low_boost_touches: g("Touches under 20 boost", "Touches made with less than 20 boost."),
  behind_ball: g(
    "Behind the ball",
    "Share of the match you were between the ball and your own goal, in position to defend or challenge.",
  ),
  first_man_share: g("Time as 1st man", "Share of the match you were the teammate closest to the ball."),
  fifty_wins: g(
    "50/50s won",
    "Share of open-play 50/50s you won: you and an opponent hit the ball at almost the same time. Kickoffs are counted apart.",
  ),
  whiffs: g("Whiffs", "Times you went for the ball, on the ground or in the air, got close, and missed it."),
  big_pads: g("Big pads", "Full boost pads (the 100 ones) you picked up, read from the replay."),
  fifties: g(
    "50/50s won–lost",
    "Open-play challenges where you and an opponent hit the ball at almost the same time. Kickoffs are counted apart.",
  ),
  kickoffs: g("Kickoffs won–lost", "Kickoff challenges you won or lost."),
  first_second_man: g(
    "1st / 2nd man",
    "Seconds you were the teammate closest to the ball (1st man) or the next closest (2nd man).",
  ),
};

export type TermKey =
  | "to_save"
  | "replay_stats"
  | "training"
  | "make_rate"
  | "best_stretch"
  | "kpi_training"
  | "kpi_make_rate"
  | "kpi_ranked"
  | "kpi_freeplay";

const MAKE_RATE = g("Make rate", "Share of your training attempts that went in, quick resets left out.");

export const TERMS: Record<TermKey, Gloss> = {
  to_save: g(
    "Replays to save",
    `Ranked matches with no replay that are still in your in-game match history (the last ${HISTORY_SIZE}). Save them from Match History and the app reads them on its own.`,
  ),
  replay_stats: g("Replay stats", "This match's replay was read. Click to see every player's stats from it."),
  training: g(
    "Training",
    "Each pack you played this session, its make rate against the pack's target, and how it went over time.",
  ),
  make_rate: MAKE_RATE,
  best_stretch: g("Best stretch", "Your best run of 20 attempts in a row, and when it happened."),
  kpi_training: g("Training", "Attempts in training packs this session."),
  kpi_make_rate: MAKE_RATE,
  kpi_ranked: g("Ranked", "Ranked wins and losses this session."),
  kpi_freeplay: g("Free play", "Minutes spent in free play this session."),
};

/** The Progress and Packs pages' tiles and charts. */
export const PROGRESS = {
  ranked: g("Ranked", "Ranked wins and losses in the range, all modes together."),
  make_rate: MAKE_RATE,
  packs_at_target: g("Packs at target", "Packs whose make rate over the range reached the pack's target."),
  played: g("Played", "Time spent in sessions over the range."),
  mmr: g(
    "MMR",
    "Your latest MMR in the range, from what you entered at the end of sessions. The small number is the change over the range.",
  ),
  record: g("Record", "Ranked wins and losses in this mode over the range."),
  shooting: MODE_STATS.shooting_pct,
  team_share: g("Team share", "Your part of your team's total score, then your team's possession of the ball."),
  latest_session: g("Latest session", "The make rate of your most recent session on this pack in the range."),
  over_range: g("Over the range", "The make rate over every attempt on this pack in the range."),
  best_session: g("Best session", "Your best session on this pack. Sessions with under 10 attempts don't count."),
  volume: g("Volume", "Attempts on this pack in the range, and the sessions and time they took."),
  pack_overall: g("Overall", "The make rate over every attempt you have recorded on this pack."),
  pack_last_session: g("Last session", "The make rate in the last session that played this pack."),
  pack_played: g("Played", "Attempts recorded on this pack, quick resets left out."),
  session_by_session: g("Session by session", "Each session that played this pack, newest first, against its target."),
  pack_chart: g(
    "Make rate",
    "Your make rate session by session. Hollow points are sessions with under 10 attempts, left out of the average line.",
  ),
  mmr_chart: g("MMR", "The MMR you entered at the end of each session."),
} satisfies Record<string, Gloss>;

/** A live-feed stat's hint, titled the way the card showing it names it. */
export const statGloss = (stat: ModeStat, title: string): Gloss => g(title, MODE_STATS[stat].text);

const REVIEW_REPLAY_KEYS: readonly string[] = ["low_boost_touches", "behind_ball", "first_man_share", "fifty_wins", "whiffs"];

/** The hint for a coach review item key (a live-feed stat or a replay stat), or null. */
export function glossFor(key: string): Gloss | null {
  if (Object.hasOwn(MODE_STATS, key)) return MODE_STATS[key as ModeStat];
  if (REVIEW_REPLAY_KEYS.includes(key)) return REPLAY_STATS[key as ReplayKey];
  return null;
}

export const usualGloss = (mode: string): Gloss =>
  g(
    "Your usual",
    `The average of your earlier ranked ${mode} matches. The grey band is your normal range; the tick is this session. Only stats far outside that range are shown.`,
  );

export const placeGloss = (place: number): Gloss =>
  g(
    "Place in match history",
    `The game keeps your last ${HISTORY_SIZE} online matches. This one is about place ${place} of ${HISTORY_SIZE}.`,
  );

export const noReplayGloss = (place: number): Gloss =>
  g(
    "No replay yet",
    `About place ${place} of ${HISTORY_SIZE} in your in-game match history. Save it from Match History before it drops out.`,
  );

export const lowBoostGloss = (low: number, all: number): Gloss =>
  g(REPLAY_STATS.low_boost_touches.title, `${REPLAY_STATS.low_boost_touches.text} ${low} of your ${all} touches.`);
