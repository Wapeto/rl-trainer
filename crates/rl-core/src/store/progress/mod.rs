//! Progress over time for the desktop Progress page (Plan 5, spec §2–§3). `rules` holds the
//! change and averaging rules; each read returns one typed struct for the UI.

pub mod list;
mod load;
pub mod mode;
pub mod overview;
pub mod pack;
pub mod rules;
pub mod stats;
#[cfg(test)]
pub(crate) mod test_db;

pub use list::{progress_list, ModeLane, PackLane, ProgressList};
pub use mode::{progress_mode, ModeProgress, StatPoint, StatSeries};
pub use overview::{progress_overview, ModeMmr, Mover, OverviewProgress};
pub use pack::{progress_pack, PackPoint, PackProgress};
pub use stats::ModeStat;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const DAY_MS: i64 = 86_400_000;
pub const WEEK_MS: i64 = 7 * DAY_MS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgressRange {
    Days30,
    Days90,
    All,
}

impl ProgressRange {
    /// First instant of the range: a session belongs to it when it started at or after this.
    pub fn start(self, now_ms: i64) -> i64 {
        match self {
            Self::Days30 => now_ms - 30 * DAY_MS,
            Self::Days90 => now_ms - 90 * DAY_MS,
            Self::All => i64::MIN,
        }
    }

    /// The start to show; `None` for All.
    pub fn shown_start(self, now_ms: i64) -> Option<i64> {
        (self != Self::All).then(|| self.start(now_ms))
    }
}

/// An MMR entry as a chart point; `session_id` is set when it was saved with a session.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct MmrPoint {
    pub session_id: Option<i64>,
    pub at: i64,
    pub mmr: i64,
}

/// One session's finished play time by kind. The UI groups sessions into local weeks.
#[derive(Debug, Clone, PartialEq, Default, Serialize, TS)]
pub struct SessionTime {
    pub session_id: i64,
    pub at: i64,
    pub training_ms: i64,
    pub freeplay_ms: i64,
    /// Matches with a ranked record; casual, private and other activities go to `other_ms`.
    pub ranked_ms: i64,
    pub other_ms: i64,
}

impl SessionTime {
    pub fn total_ms(&self) -> i64 {
        self.training_ms + self.freeplay_ms + self.ranked_ms + self.other_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_start_30_or_90_days_back_and_all_has_no_start() {
        assert_eq!(ProgressRange::Days30.start(100 * DAY_MS), 70 * DAY_MS);
        assert_eq!(
            ProgressRange::Days90.shown_start(100 * DAY_MS),
            Some(10 * DAY_MS)
        );
        assert_eq!(ProgressRange::All.shown_start(100 * DAY_MS), None);
        assert_eq!(
            serde_json::to_string(&ProgressRange::Days90).unwrap(),
            "\"days90\""
        );
    }
}
