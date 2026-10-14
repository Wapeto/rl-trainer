//! Mode detection from `UpdateState.Game.PlaylistId`.

use ts_rs::TS;

use crate::events::UpdateState;

pub const FREEPLAY: i32 = 9;
pub const CUSTOM_TRAINING: i32 = 21;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum Activity {
    Freeplay,
    Training,
    Match,
    Other,
}

impl Activity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Freeplay => "freeplay",
            Self::Training => "training",
            Self::Match => "match",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaylistInfo {
    pub ranked: bool,
    pub team_size: Option<u8>,
}

/// Only IDs confirmed from recorded data (see spikes/stats-api/FINDINGS.md). Add rows as they
/// are verified; unknown IDs are treated as unranked with team size inferred from players.
pub fn playlist_info(id: Option<i32>) -> PlaylistInfo {
    let (ranked, team_size) = match id {
        Some(2) => (false, Some(2)),
        Some(10) => (true, Some(1)),
        Some(11) => (true, Some(2)),
        Some(13) => (true, Some(3)),
        _ => (false, None),
    };
    PlaylistInfo { ranked, team_size }
}

pub fn classify(update: &UpdateState, in_saved_replay: bool) -> Activity {
    if in_saved_replay {
        return Activity::Other;
    }
    match update.game.playlist_id {
        Some(FREEPLAY) => Activity::Freeplay,
        Some(CUSTOM_TRAINING) => Activity::Training,
        _ if !update.match_guid.is_empty() => Activity::Match,
        _ => Activity::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{GameState, UpdateState};

    fn update(playlist: Option<i32>, guid: &str) -> UpdateState {
        UpdateState {
            match_guid: guid.into(),
            game: GameState {
                playlist_id: playlist,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn classifies_modes() {
        assert_eq!(classify(&update(Some(9), ""), false), Activity::Freeplay);
        assert_eq!(classify(&update(Some(21), ""), false), Activity::Training);
        assert_eq!(classify(&update(Some(2), "G"), false), Activity::Match);
        assert_eq!(classify(&update(Some(6), ""), false), Activity::Other);
    }

    #[test]
    fn saved_replays_are_never_matches() {
        assert_eq!(classify(&update(Some(13), "G"), true), Activity::Other);
    }

    #[test]
    fn only_confirmed_playlists_are_labelled() {
        assert_eq!(
            playlist_info(Some(10)),
            PlaylistInfo {
                ranked: true,
                team_size: Some(1)
            }
        );
        assert_eq!(
            playlist_info(Some(11)),
            PlaylistInfo {
                ranked: true,
                team_size: Some(2)
            }
        );
        assert_eq!(
            playlist_info(Some(2)),
            PlaylistInfo {
                ranked: false,
                team_size: Some(2)
            }
        );
        assert_eq!(
            playlist_info(Some(13)),
            PlaylistInfo {
                ranked: true,
                team_size: Some(3)
            }
        );
        assert_eq!(
            playlist_info(Some(999)),
            PlaylistInfo {
                ranked: false,
                team_size: None
            }
        );
        assert_eq!(
            playlist_info(None),
            PlaylistInfo {
                ranked: false,
                team_size: None
            }
        );
    }

    #[test]
    fn activity_names_match_the_schema() {
        let names: Vec<_> = [
            Activity::Freeplay,
            Activity::Training,
            Activity::Match,
            Activity::Other,
        ]
        .iter()
        .map(|a| a.as_str())
        .collect();
        assert_eq!(names, ["freeplay", "training", "match", "other"]);
    }
}
