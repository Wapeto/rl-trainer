// Generated from the Rust types; do not edit.
// Regenerate: UPDATE_GENERATED=1 cargo test -p rl-app ts_export

export type Activity = "freeplay" | "training" | "match" | "other";

export type PackRow = { id: number, code: string, name: string, creator: string, tags: Array<string>, shot_count: number | null, target_rate: number, 
/**
 * Hidden packs keep their history but leave the phone's picker and suggestions.
 */
hidden: boolean, notes: string, 
/**
 * Goalie pack: a shot is made unless the ball goes in.
 */
goalie: boolean, };

export type PackStats = { pack: PackRow, 
/**
 * All-time attempts on the pack, skipped ones excluded.
 */
attempts: number, makes: number, make_rate: number | null, last_trained_at: number | null, };

export type MmrInput = { team_size: number, mmr: number, };

export type RecentAttempt = { id: number, seq: number, outcome: string, skipped: boolean, };

export type Nudge = { "kind": "rate_drop", fired_at_attempt: number, last20_rate: number, best20_rate: number, } | { "kind": "miss_streak", fired_at_attempt: number, count: number, };

export type LiveTraining = { attempts: number, makes: number, make_rate: number | null, streak: number, 
/**
 * Up to the last 10 attempts, oldest first.
 */
recent: Array<RecentAttempt>, 
/**
 * The nudge to show on the phone now (spec §3.2).
 */
nudge: Nudge | null, };

export type LiveFreeplay = { touches: number, hard_hits: number, max_hit_speed: number, goals: number, pct_supersonic: number, pct_on_wall: number, pct_airborne: number, 
/**
 * Hardest hit across today's free play, the current stretch included.
 */
best_today_hit: number, };

export type CompareStat = "score" | "goals" | "assists" | "saves" | "shots";

export type StatCompare = { stat: CompareStat, mine: number, best: number, 
/**
 * Set only when exactly one other player holds `best`.
 */
best_name: string | null, 
/**
 * Several players share `best`.
 */
tie: boolean, 
/**
 * You hold `best` alone.
 */
mine_is_best: boolean, };

export type LiveMatch = { playlist_id: number | null, ranked: boolean, team_size: number | null, arena: string, team_score: number, opponent_score: number, clock_seconds: number, overtime: boolean, goals: number, assists: number, saves: number, shots: number, score: number, 
/**
 * Your hardest ball hit this match (km/h).
 */
hardest_hit: number, pct_supersonic: number, compare: Array<StatCompare>, avg_boost: number, pct_zero_boost: number, today_wins: number, today_losses: number, };

export type LiveSession = { id: number, started_at: number, notes: string, };

export type LiveBlock = { id: number, kind: Activity, playlist_id: number | null, started_at: number, paused: boolean, pack: PackRow | null, suggested_pack: PackRow | null, training: LiveTraining | null, freeplay: LiveFreeplay | null, match: LiveMatch | null, };

export type MmrLatest = { team_size: number, mmr: number, recorded_at: number, };

export type SessionSummary = { id: number, started_at: number, ended_at: number, training_blocks: number, freeplay_blocks: number, match_blocks: number, other_blocks: number, wins: number, losses: number, 
/**
 * Ranked team sizes played since the last review: one MMR prompt each.
 */
ranked_team_sizes: Array<number>, notes: string, 
/**
 * Free-play stretches, packs and matches in the session.
 */
activities: number, training_make_rate: number | null, 
/**
 * Average target of the packs trained in the session.
 */
training_target_rate: number | null, reviewed: boolean, };

export type LiveSnapshot = { session: LiveSession | null, block: LiveBlock | null, latest_mmr: Array<MmrLatest>, last_session: SessionSummary | null, 
/**
 * The open session so far (its `ended_at` is when its latest activity ended).
 */
current_session: SessionSummary | null, };

export type StatsApiStatus = { "state": "unknown" } | { "state": "ok", rate: number, port: number, web_port: number | null, } | { "state": "disabled", ini: string, } | { "state": "file_missing", ini: string, } | { "state": "unreadable", ini: string, error: string, } | { "state": "install_not_found" };

export type AppSnapshot = { version: number, 
/**
 * +1 each time an engine starts (startup, restore): windows reload everything on a change.
 */
generation: number, game_running: boolean, ingest_connected: boolean, stats_api: StatsApiStatus, phone_clients: number, 
/**
 * +1 per imported replay (spec §4): part of the windows' reload key.
 */
replays: number, live: LiveSnapshot, };

export type ReviewBody = { mmr: Array<MmrInput>, notes: string | null, };

export type SessionCard = { id: number, started_at: number, ended_at: number | null, 
/**
 * Distinct packs trained; training without a pack counts as one.
 */
packs: number, ranked_matches: number, casual_matches: number, freeplay_activities: number, needs_review: boolean, 
/**
 * Built from replay files, not recorded live.
 */
imported: boolean, };

export type PackRef = { id: number, name: string, code: string, target_rate: number, };

export type FreeplaySummary = { touches: number, hard_hits: number, max_hit_speed: number, };

export type ReplayStats = { tracked_seconds: number, pct_behind_ball: number, pct_in_front_of_ball: number, pct_defensive_third: number, pct_neutral_third: number, pct_offensive_third: number, 
/**
 * Team modes only.
 */
secs_first_man: number | null, secs_second_man: number | null, secs_third_man: number | null, became_first_man: number | null, big_pads: number, small_pads: number, pads_stolen: number, touches: number, touches_low_boost: number, touches_zero_boost: number, fifty_wins: number, fifty_losses: number, fifty_low_boost: number, kickoff_wins: number, kickoff_losses: number, whiffs: number, };

export type PlayerReplayStats = { 
/**
 * Seconds the replay tracked this player (positioning); per-5-minute values divide by it.
 */
tracked_seconds: number | null, score: number | null, goals: number | null, assists: number | null, saves: number | null, shots: number | null, pct_behind_ball: number | null, pct_in_front_of_ball: number | null, pct_defensive_third: number | null, pct_neutral_third: number | null, pct_offensive_third: number | null, pct_most_back: number | null, pct_shadow_defense: number | null, avg_distance_to_ball_m: number | null, secs_first_man: number | null, secs_second_man: number | null, secs_third_man: number | null, became_first_man: number | null, boost_collected: number | null, boost_used: number | null, boost_stolen: number | null, big_pads: number | null, small_pads: number | null, pct_zero_boost: number | null, pct_full_boost: number | null, boost_used_supersonic: number | null, touches_low_boost: number | null, touches: number | null, hard_hits: number | null, aerial_touches: number | null, control_touches: number | null, first_touches: number | null, fifty_wins: number | null, fifty_losses: number | null, fifty_neutral: number | null, kickoff_wins: number | null, kickoff_losses: number | null, beaten_to_ball: number | null, whiffs: number | null, flicks: number | null, wavedashes: number | null, half_flips: number | null, speed_flips: number | null, avg_speed_kmh: number | null, pct_supersonic: number | null, pct_on_ground: number | null, pct_high_air: number | null, demos_inflicted: number | null, demos_taken: number | null, bumps_inflicted: number | null, bumps_taken: number | null, passes_completed: number | null, passes_received: number | null, };

export type ReplayPlayer = { slot: number, team: number, name: string, is_me: boolean, is_bot: boolean, stats: PlayerReplayStats, };

export type Insight = { 
/**
 * The page row it points to.
 */
stat: string, fact: string, tip: string, };

export type BenchStat = { 
/**
 * (winners − losers) / pooled sd: positive means more is better.
 */
gap: number, separates: boolean, 
/**
 * My percentile in the pool, flipped when less is better, so higher is always better.
 */
me: number | null, 
/**
 * The winners' mean percentile, on the same scale.
 */
winners_pct: number, };

export type Benchmark = { 
/**
 * Matches in the pool (the page says how many more are needed).
 */
matches: number, 
/**
 * Winners' means, each row scaled to 5 minutes first; `None` under `MIN_POOL_MATCHES`.
 */
winners: PlayerReplayStats | null, losers: PlayerReplayStats | null, 
/**
 * Per stat by field name; empty under `MIN_POOL_MATCHES`.
 */
stats: { [key in string]: BenchStat }, };

export type MatchReplay = { players: Array<ReplayPlayer>, benchmark: Benchmark, insights: Array<Insight>, };

export type MatchSummary = { ranked: boolean, team_size: number | null, arena: string, 
/**
 * `win`, `loss` or `unknown`.
 */
result: string, team_score: number, opponent_score: number, overtime: boolean, score: number, goals: number, assists: number, saves: number, match_id: number, 
/**
 * About where the match sits in the game's match history (1 = newest; over 20 = gone).
 */
history_place: number, 
/**
 * My replay stats, when its replay was imported.
 */
replay: ReplayStats | null, };

export type ActivityRow = { id: number, kind: Activity, playlist_id: number | null, started_at: number, 
/**
 * `None` while it is being recorded.
 */
ended_at: number | null, pack: PackRef | null, 
/**
 * Training only; skipped attempts left out.
 */
attempts: number, makes: number, freeplay: FreeplaySummary | null, match_summary: MatchSummary | null, };

export type MmrRow = { id: number, team_size: number, mmr: number, recorded_at: number, 
/**
 * Against the previous entry for the same team size (any session); `None` for the first.
 */
change: number | null, };

export type SessionTotals = { attempts: number, makes: number, 
/**
 * Targets of the trained packs weighted by attempts (no pack: `DEFAULT_TARGET_RATE`).
 */
target_rate: number | null, 
/**
 * Ranked matches only.
 */
wins: number, losses: number, freeplay_ms: number, };

export type SessionDetail = { id: number, started_at: number, ended_at: number | null, notes: string, needs_review: boolean, previous_id: number | null, next_id: number | null, activities: Array<ActivityRow>, mmr: Array<MmrRow>, totals: SessionTotals, };

export type ReplayToSave = { match_id: number, session_id: number, started_at: number, ranked: boolean, team_size: number | null, result: string, team_score: number, opponent_score: number, 
/**
 * About where it sits in the game's match history (1 = newest).
 */
place: number, };

export type ReplayCounts = { imported: number, unmatched: number, not_mine: number, errors: number, last_error: string | null, 
/**
 * The last 3 replays of recorded matches all failed: the format may have changed.
 */
format_warning: boolean, };

export type ReviewUnit = "percent" | "per_game" | "kmh" | "boost" | "per_minute";

export type TrainingItem = { pack_id: number | null, pack_name: string | null, 
/**
 * The first attempt on the pack this session.
 */
started_at: number, makes: number, counted: number, quick_resets: number, make_rate: number | null, target_rate: number, 
/**
 * Makes in the best 20-attempt stretch, and when it ended.
 */
best_makes: number | null, best_at: number | null, 
/**
 * When the rate nudge first fired.
 */
dropped_at: number | null, 
/**
 * Make rate per 10 counted attempts, oldest first.
 */
bars: Array<number>, };

export type ReviewItem = { key: string, label: string, unit: ReviewUnit, value: number, usual: number, better: boolean, 
/**
 * How widely the stat usually varies (the standard deviation of the earlier matches).
 */
spread: number, 
/**
 * Matches of the session the value covers, out of `of` (replay stats: those with a replay).
 */
covered: number, of: number, };

export type ModeReview = { team_size: number, matches: number, 
/**
 * Earlier ranked matches of this mode used as "usual" (up to 20).
 */
earlier: number, items: Array<ReviewItem>, };

export type SessionReview = { session_id: number, training: Array<TrainingItem>, modes: Array<ModeReview>, to_save: Array<ReplayToSave>, 
/**
 * When a replay of this session arrived after it ended (the review changed).
 */
replay_added_at: number | null, };

export type ReplayStatus = { 
/**
 * The folders being watched (none when the game's folder was not found).
 */
folders: Array<string>, counts: ReplayCounts, };

export type Neighbour = "previous" | "next";

export type PackSort = "recent" | "name" | "make_rate" | "most_played";

export type PackInput = { code: string, name: string, 
/**
 * 0.01–1.
 */
target_rate: number, shot_count: number | null, tags: Array<string>, notes: string, 
/**
 * Goalie pack: a shot is made unless the ball goes in.
 */
goalie: boolean, };

export type PackSessionRow = { session_id: number, started_at: number, attempts: number, makes: number, make_rate: number, };

export type PackDetail = { stats: PackStats, last_session_rate: number | null, 
/**
 * Best rate among sessions with at least `BEST_SESSION_MIN_ATTEMPTS` attempts.
 */
best_session_rate: number | null, 
/**
 * One row per session that played the pack, newest first.
 */
history: Array<PackSessionRow>, };

export type Settings = { phone_enabled: boolean, phone_port: number, session_timeout_min: number, hard_hit_kmh: number, install_dir: string | null, 
/**
 * The replay folder chosen in Settings; `None` uses the game's folders.
 */
replay_dir: string | null, firewall_port: number | null, dev_db_imported_at: number | null, 
/**
 * What the user typed as "me": an in-game name or an account id.
 */
me_hint: string | null, 
/**
 * The "Get started" card on Sessions was dismissed.
 */
setup_dismissed: boolean, 
/**
 * Sends a random install id and the app version to the relay once a day.
 */
usage_ping: boolean, };

export type BackupKind = "daily" | "pre_migration" | "pre_restore" | "pre_edit";

export type BackupInfo = { name: string, kind: BackupKind, size_bytes: number, modified_ms: number, };

export type PairingInfo = { url: string, port: number, addresses: Array<string>, qr_svg: string, };

export type StartupReport = { imported_dev_db: boolean, pre_migration_backup: string | null, sessions_recovered: number, daily_backup_error: string | null, };

export type ProfileRow = { primary_id: string, display_name: string, is_active: boolean, };

export type About = { version: string, 
/**
 * Where the database, backups and logs live.
 */
data_dir: string, 
/**
 * A debug build: it never registers itself to start with Windows.
 */
dev_build: boolean, };

export type ReportInfo = { 
/**
 * False in builds without a relay address: the page only offers saving.
 */
can_send: boolean, };

export type ProgressRange = "days30" | "days90" | "all";

export type MmrPoint = { session_id: number | null, at: number, mmr: number, };

export type SessionTime = { session_id: number, at: number, training_ms: number, freeplay_ms: number, 
/**
 * Matches with a ranked record; casual, private and other activities go to `other_ms`.
 */
ranked_ms: number, other_ms: number, };

export type PackPoint = { session_id: number, 
/**
 * When the session started.
 */
at: number, makes: number, attempts: number, 
/**
 * Time on the pack in that session.
 */
ms: number, 
/**
 * makes ÷ attempts, 0–1.
 */
rate: number, 
/**
 * Under 10 attempts: drawn hollow, left out of the average line and of the best session.
 */
thin: boolean, 
/**
 * Make rate over this session and the two previous ones that are not thin; `None` when thin.
 */
average: number | null, };

export type PackProgress = { pack: PackRow, 
/**
 * Oldest first.
 */
points: Array<PackPoint>, makes: number, attempts: number, ms: number, 
/**
 * All makes ÷ all attempts in the range.
 */
rate: number | null, 
/**
 * The latest session's rate minus the first one's in the range (2+ sessions).
 */
latest_change: number | null, 
/**
 * Last 3 sessions minus first 3 (4+ sessions).
 */
change: number | null, best: PackPoint | null, };

export type ModeStat = "win_rate" | "goals" | "assists" | "saves" | "shots" | "score" | "shooting_pct" | "goal_speed" | "epic_saves" | "touches" | "aerial_touches" | "hit_power" | "hard_hits" | "defensive_third" | "attacking_third" | "airborne" | "on_wall" | "avg_speed" | "supersonic" | "avg_boost" | "zero_boost" | "full_boost" | "boosting" | "boost_per_minute" | "big_pads" | "demos_given" | "demos_taken" | "share_score" | "share_goals" | "share_saves" | "possession";

export type StatPoint = { session_id: number, at: number, value: number, 
/**
 * The session's ranked matches of the mode and their results (for the tooltip).
 */
matches: number, wins: number, losses: number, 
/**
 * A match of the session was joined late: its time-based stats are partial.
 */
partial: boolean, };

export type StatSeries = { stat: ModeStat, 
/**
 * Sessions with a value for the stat, oldest first.
 */
points: Array<StatPoint>, overall: number | null, change: number | null, 
/**
 * For a stat new in Plan 5 when older matches of the range lack it: the first match with it.
 */
since: number | null, 
/**
 * Matches that count toward the stat; for a Plan 5 stat, matches with the Plan 5 stats recorded.
 */
matches: number, };

export type ModeProgress = { team_size: number, matches: number, sessions: number, wins: number, losses: number, overtimes: number, 
/**
 * MMR entries of the range, oldest first.
 */
mmr: Array<MmrPoint>, mmr_latest: number | null, mmr_change: number | null, mmr_first: number | null, mmr_high: number | null, 
/**
 * One series per `ModeStat::ALL`, in that order.
 */
stats: Array<StatSeries>, };

export type ModeLane = { team_size: number, matches: number, wins: number, losses: number, mmr_latest: number | null, mmr_change: number | null, 
/**
 * MMR entries of the range, oldest first.
 */
mmr_spark: Array<number>, };

export type PackLane = { pack: PackRow, sessions: number, attempts: number, 
/**
 * Make rate over the range, 0–1.
 */
rate: number, change: number | null, 
/**
 * Per-session make rates, oldest first.
 */
spark: Array<number>, last_played: number, };

export type ProgressList = { sessions: number, played_ms: number, 
/**
 * Most ranked matches first.
 */
modes: Array<ModeLane>, 
/**
 * Most attempts first, then by name.
 */
packs: Array<PackLane>, };

export type ModeMmr = { team_size: number, points: Array<MmrPoint>, };

export type Mover = { pack: PackRow, 
/**
 * Make rates (0–1): see `rules::mover_span`.
 */
from: number, to: number, sessions: number, };

export type OverviewProgress = { 
/**
 * Start of the range; `None` for All.
 */
start: number | null, sessions: number, played_ms: number, 
/**
 * Play time per week over the range (All: since the first session).
 */
per_week_ms: number, ranked_matches: number, wins: number, losses: number, 
/**
 * Counted attempts on packs.
 */
attempts: number, make_rate: number | null, make_rate_change: number | null, packs_played: number, packs_at_target: number, mmr: Array<ModeMmr>, modes: Array<ModeLane>, 
/**
 * Up to four packs with the largest change, largest first.
 */
movers: Array<Mover>, 
/**
 * Oldest first; the UI groups them into weeks.
 */
time: Array<SessionTime>, };

