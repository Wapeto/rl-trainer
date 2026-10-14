//! Live state for the phone and desktop UIs. This JSON shape is the contract Plan 3 builds on.

use serde::Serialize;
use ts_rs::TS;

use crate::playlist::Activity;
use crate::store::PackRow;

#[derive(Debug, Clone, PartialEq, Serialize, Default, TS)]
pub struct LiveSnapshot {
    pub session: Option<LiveSession>,
    pub block: Option<LiveBlock>,
    pub latest_mmr: Vec<MmrLatest>,
    pub last_session: Option<SessionSummary>,
    /// The open session so far (its `ended_at` is when its latest activity ended).
    pub current_session: Option<SessionSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default, TS)]
pub struct LiveSession {
    pub id: i64,
    pub started_at: i64,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct LiveBlock {
    pub id: i64,
    pub kind: Activity,
    pub playlist_id: Option<i32>,
    pub started_at: i64,
    pub paused: bool,
    pub pack: Option<PackRow>,
    pub suggested_pack: Option<PackRow>,
    pub training: Option<LiveTraining>,
    pub freeplay: Option<LiveFreeplay>,
    #[serde(rename = "match")]
    pub match_live: Option<LiveMatch>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default, TS)]
pub struct LiveTraining {
    pub attempts: i64,
    pub makes: i64,
    pub make_rate: Option<f64>,
    pub streak: i64,
    /// Up to the last 10 attempts, oldest first.
    pub recent: Vec<RecentAttempt>,
    /// The nudge to show on the phone now (spec §3.2).
    pub nudge: Option<crate::coach::nudges::Nudge>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default, TS)]
pub struct RecentAttempt {
    pub id: i64,
    pub seq: i64,
    pub outcome: String,
    pub skipped: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default, TS)]
pub struct LiveFreeplay {
    pub touches: u32,
    pub hard_hits: u32,
    pub max_hit_speed: f64,
    pub goals: u32,
    pub pct_supersonic: f64,
    pub pct_on_wall: f64,
    pub pct_airborne: f64,
    /// Hardest hit across today's free play, the current stretch included.
    pub best_today_hit: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CompareStat {
    Score,
    Goals,
    Assists,
    Saves,
    Shots,
}

/// Your value of one scoreboard stat against the best in the lobby (players who left included).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct StatCompare {
    pub stat: CompareStat,
    pub mine: i32,
    pub best: i32,
    /// Set only when exactly one other player holds `best`.
    pub best_name: Option<String>,
    /// Several players share `best`.
    pub tie: bool,
    /// You hold `best` alone.
    pub mine_is_best: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default, TS)]
pub struct LiveMatch {
    pub playlist_id: Option<i32>,
    pub ranked: bool,
    pub team_size: Option<u8>,
    pub arena: String,
    pub team_score: i32,
    pub opponent_score: i32,
    pub clock_seconds: i32,
    pub overtime: bool,
    pub goals: i32,
    pub assists: i32,
    pub saves: i32,
    pub shots: i32,
    pub score: i32,
    /// Your hardest ball hit this match (km/h).
    pub hardest_hit: f64,
    pub pct_supersonic: f64,
    pub compare: Vec<StatCompare>,
    pub avg_boost: f64,
    pub pct_zero_boost: f64,
    pub today_wins: i64,
    pub today_losses: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default, TS)]
pub struct MmrLatest {
    pub team_size: u8,
    pub mmr: i32,
    pub recorded_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default, TS)]
pub struct SessionSummary {
    pub id: i64,
    pub started_at: i64,
    pub ended_at: i64,
    pub training_blocks: i64,
    pub freeplay_blocks: i64,
    pub match_blocks: i64,
    pub other_blocks: i64,
    pub wins: i64,
    pub losses: i64,
    /// Ranked team sizes played since the last review: one MMR prompt each.
    pub ranked_team_sizes: Vec<u8>,
    pub notes: String,
    /// Free-play stretches, packs and matches in the session.
    pub activities: i64,
    pub training_make_rate: Option<f64>,
    /// Average target of the packs trained in the session.
    pub training_target_rate: Option<f64>,
    pub reviewed: bool,
}

/// What the tracker alone knows about the open block.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackerLive {
    pub activity: Activity,
    pub playlist_id: Option<i32>,
    pub started_at_ms: i64,
    pub paused: bool,
    pub freeplay: Option<LiveFreeplay>,
    pub match_live: Option<LiveMatch>,
}
