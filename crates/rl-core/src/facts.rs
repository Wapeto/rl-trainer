//! Domain facts emitted by the tracker and persisted by the recorder.

use crate::playlist::Activity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptOutcome {
    Make,
    Miss,
}

impl AttemptOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Make => "make",
            Self::Miss => "miss",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attempt {
    pub seq: u32,
    pub started_at_ms: i64,
    pub duration_ms: i64,
    pub outcome: AttemptOutcome,
    pub retry_after_make: bool,
    pub goal_speed: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FreeplayStats {
    pub touches: u32,
    pub hard_hits: u32,
    pub avg_hit_speed: f64,
    pub max_hit_speed: f64,
    pub goals: u32,
    pub crossbar_hits: u32,
    pub pct_supersonic: f64,
    pub pct_on_wall: f64,
    pub pct_airborne: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MatchResult {
    Win,
    Loss,
    #[default]
    Unknown,
}

impl MatchResult {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Win => "win",
            Self::Loss => "loss",
            Self::Unknown => "unknown",
        }
    }
}

/// My own play in a match beyond the scoreboard (Plan 5). Percentages are of `play_seconds`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MatchMetrics {
    pub play_seconds: f64,
    /// Average `PostHitSpeed` of my touches, km/h; `None` without a touch.
    pub hit_avg_speed: Option<f64>,
    pub hard_hits: i32,
    pub aerial_touches: i32,
    pub touches_def: i32,
    pub touches_mid: i32,
    pub touches_att: i32,
    pub pct_on_ground: f64,
    pub pct_on_wall: f64,
    pub pct_airborne: f64,
    pub avg_speed: f64,
    pub pct_boosting: f64,
    pub pct_full_boost: f64,
    /// Boost spent while boosting (estimated from samples).
    pub boost_used: f64,
    /// Estimated big pads: boost rises of `BIG_PAD_RISE` or more.
    pub big_pads: i32,
    /// Average speed of my goals; `None` when I scored none.
    pub goal_speed_avg: Option<f64>,
    pub demos_taken: i32,
    /// Play time with the last touch by my team.
    pub pct_possession: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MatchRecord {
    pub match_guid: String,
    pub playlist_id: Option<i32>,
    pub ranked: bool,
    pub team_size: Option<u8>,
    pub arena: String,
    pub result: MatchResult,
    pub team_score: i32,
    pub opponent_score: i32,
    pub overtime: bool,
    pub score: i32,
    pub goals: i32,
    pub assists: i32,
    pub saves: i32,
    pub shots: i32,
    pub touches: i32,
    pub demos: i32,
    pub epic_saves: i32,
    pub avg_boost: f64,
    pub pct_zero_boost: f64,
    pub pct_supersonic: f64,
    pub metrics_partial: bool,
    pub scoreboard_json: String,
    /// `None` when none of my play was sampled.
    pub metrics: Option<MatchMetrics>,
}

// Facts are short-lived (one per event, consumed at once); boxing the match record buys nothing.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum Fact {
    ProfileDetected {
        at_ms: i64,
        primary_id: String,
        display_name: String,
    },
    SessionStarted {
        at_ms: i64,
    },
    SessionEnded {
        at_ms: i64,
    },
    BlockStarted {
        at_ms: i64,
        activity: Activity,
        playlist_id: Option<i32>,
        /// Set when the block was opened by a pack change (training only).
        pack_id: Option<i64>,
    },
    BlockEnded {
        at_ms: i64,
        data_gap_seconds: i64,
    },
    AttemptFinished(Attempt),
    FreeplayFinished(FreeplayStats),
    MatchFinished(MatchRecord),
}
