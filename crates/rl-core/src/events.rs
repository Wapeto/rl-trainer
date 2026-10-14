//! Typed Stats API events. Only fields the engine uses are modelled; unknown ones are ignored.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct PlayerRef {
    pub name: String,
    pub shortcut: i32,
    pub team_num: i32,
}

/// One player in `UpdateState`. Booleans and boost are omitted by the game when false/zero
/// (and for opponents when hidden); the engine only reads them for its own player.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct PlayerState {
    pub name: String,
    pub primary_id: String,
    pub shortcut: i32,
    pub team_num: i32,
    pub score: i32,
    pub goals: i32,
    pub shots: i32,
    pub assists: i32,
    pub saves: i32,
    pub touches: i32,
    pub car_touches: i32,
    pub demos: i32,
    pub boost: f64,
    pub speed: f64,
    #[serde(rename = "bOnGround")]
    pub on_ground: bool,
    #[serde(rename = "bOnWall")]
    pub on_wall: bool,
    #[serde(rename = "bSupersonic")]
    pub supersonic: bool,
    #[serde(rename = "bBoosting")]
    pub boosting: bool,
    #[serde(rename = "bDemolished")]
    pub demolished: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// `UpdateState.Game.Ball`; only the last-touch team is used.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BallState {
    /// Team of the last touch: 0 or 1, or 255 before anyone touched the ball.
    #[serde(default = "no_team")]
    pub team_num: i32,
}

fn no_team() -> i32 {
    255
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct TeamState {
    pub name: String,
    pub team_num: i32,
    pub score: i32,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct GameState {
    pub teams: Vec<TeamState>,
    pub time_seconds: f64,
    pub arena: String,
    pub playlist_id: Option<i32>,
    #[serde(rename = "bOvertime")]
    pub overtime: bool,
    #[serde(rename = "bReplay")]
    pub replay: bool,
    #[serde(rename = "bHasTarget")]
    pub has_target: bool,
    pub target: Option<PlayerRef>,
    #[serde(rename = "bHasWinner")]
    pub has_winner: bool,
    /// The winning team's name (matches `TeamState::name`), once the game has one.
    pub winner: String,
    pub ball: Option<BallState>,
}

impl GameState {
    pub fn team_score(&self, team_num: i32) -> i32 {
        self.teams
            .iter()
            .find(|t| t.team_num == team_num)
            .map_or(0, |t| t.score)
    }

    /// The team that touched the ball last, once someone has.
    pub fn last_touch_team(&self) -> Option<i32> {
        self.ball
            .as_ref()
            .map(|b| b.team_num)
            .filter(|t| *t == 0 || *t == 1)
    }
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct UpdateState {
    pub match_guid: String,
    pub players: Vec<PlayerState>,
    pub game: GameState,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct HitBall {
    pub pre_hit_speed: f64,
    pub post_hit_speed: f64,
    pub location: Option<Vec3>,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct BallHit {
    pub players: Vec<PlayerRef>,
    pub ball: HitBall,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct GoalScored {
    pub goal_speed: f64,
    pub scorer: Option<PlayerRef>,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Statfeed {
    pub event_name: String,
    pub main_target: Option<PlayerRef>,
    /// The victim of a `Demolish` (the attacker is `main_target`).
    pub secondary_target: Option<PlayerRef>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
struct MatchEndedData {
    winner_team_num: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GameEvent {
    UpdateState(UpdateState),
    MatchCreated {
        match_guid: String,
    },
    MatchInitialized {
        match_guid: String,
    },
    MatchDestroyed,
    MatchEnded {
        winner_team_num: Option<i32>,
    },
    MatchPaused,
    MatchUnpaused,
    RoundStarted,
    CountdownBegin,
    GoalScored(GoalScored),
    BallHit(BallHit),
    CrossbarHit,
    Statfeed(Statfeed),
    GoalReplayStart,
    GoalReplayEnd,
    ReplayCreated,
    /// Any event the engine does not use (PlayerJoined, PodiumStart, …).
    Other(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimedEvent {
    pub at_ms: i64,
    pub event: GameEvent,
}

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("message has no Event field")]
    MissingEvent,
}

/// Decode one framed message `{"Event": ..., "Data": ...}`; `Data` may be a JSON-encoded string.
pub fn decode(text: &str) -> Result<GameEvent, DecodeError> {
    let envelope: Value = serde_json::from_str(text)?;
    let name = envelope
        .get("Event")
        .and_then(Value::as_str)
        .ok_or(DecodeError::MissingEvent)?;
    let data = match envelope.get("Data") {
        Some(Value::String(s)) => serde_json::from_str(s)?,
        Some(v) => v.clone(),
        None => Value::Null,
    };
    decode_parts(name, data)
}

/// Decode an event whose `Data` has already been parsed (used for recordings).
pub fn decode_parts(name: &str, data: Value) -> Result<GameEvent, DecodeError> {
    let match_guid = data
        .get("MatchGuid")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    Ok(match name {
        "UpdateState" => GameEvent::UpdateState(serde_json::from_value(data)?),
        "MatchCreated" => GameEvent::MatchCreated { match_guid },
        "MatchInitialized" => GameEvent::MatchInitialized { match_guid },
        "MatchDestroyed" => GameEvent::MatchDestroyed,
        "MatchEnded" => {
            let ended: MatchEndedData = serde_json::from_value(data)?;
            GameEvent::MatchEnded {
                winner_team_num: ended.winner_team_num,
            }
        }
        "MatchPaused" => GameEvent::MatchPaused,
        "MatchUnpaused" => GameEvent::MatchUnpaused,
        "RoundStarted" => GameEvent::RoundStarted,
        "CountdownBegin" => GameEvent::CountdownBegin,
        "GoalScored" => GameEvent::GoalScored(serde_json::from_value(data)?),
        "BallHit" => GameEvent::BallHit(serde_json::from_value(data)?),
        "CrossbarHit" => GameEvent::CrossbarHit,
        "StatfeedEvent" => GameEvent::Statfeed(serde_json::from_value(data)?),
        "GoalReplayStart" => GameEvent::GoalReplayStart,
        "GoalReplayEnd" => GameEvent::GoalReplayEnd,
        "ReplayCreated" => GameEvent::ReplayCreated,
        other => GameEvent::Other(other.to_owned()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const UPDATE: &str = r#"{"Event":"UpdateState","Data":{"MatchGuid":"G","Players":[{"Name":"Me","PrimaryId":"Epic|me|0","Shortcut":1,"TeamNum":0,"Score":10,"Goals":1,"Shots":2,"Assists":0,"Saves":0,"Touches":5,"CarTouches":1,"Demos":0,"Speed":55.5,"Boost":33,"bOnGround":true,"bSupersonic":true}],"Game":{"Teams":[{"Name":"Blue","TeamNum":0,"Score":1},{"Name":"Orange","TeamNum":1,"Score":4}],"TimeSeconds":290,"bOvertime":false,"Ball":{"Speed":0,"TeamNum":255},"bReplay":false,"bHasWinner":false,"Winner":"","Arena":"Stadium_P","PlaylistId":21,"bHasTarget":true,"Target":{"Name":"Me","Shortcut":1,"TeamNum":0}}}}"#;

    #[test]
    fn decodes_update_state() {
        let GameEvent::UpdateState(u) = decode(UPDATE).unwrap() else {
            panic!("wrong variant")
        };
        assert_eq!(u.match_guid, "G");
        assert_eq!(u.game.playlist_id, Some(21));
        assert_eq!(u.game.time_seconds, 290.0);
        assert_eq!(u.game.team_score(1), 4);
        assert_eq!(u.game.target.as_ref().unwrap().name, "Me");
        let me = &u.players[0];
        assert_eq!(
            (me.primary_id.as_str(), me.goals, me.boost),
            ("Epic|me|0", 1, 33.0)
        );
        assert!(me.on_ground && me.supersonic);
        assert!(!me.on_wall, "absent booleans default to false");
    }

    #[test]
    fn decodes_string_encoded_data() {
        let text = r#"{"Event":"MatchEnded","Data":"{\"MatchGuid\":\"G\",\"WinnerTeamNum\":1}"}"#;
        assert_eq!(
            decode(text).unwrap(),
            GameEvent::MatchEnded {
                winner_team_num: Some(1)
            }
        );
    }

    #[test]
    fn decodes_hits_goals_and_statfeed() {
        let hit = r#"{"Event":"BallHit","Data":{"MatchGuid":"","Players":[{"Name":"Me","Shortcut":1,"TeamNum":0}],"Ball":{"PreHitSpeed":0,"PostHitSpeed":56.9,"Location":{"X":1,"Y":2,"Z":3}}}}"#;
        let GameEvent::BallHit(h) = decode(hit).unwrap() else {
            panic!()
        };
        assert_eq!(
            (h.players[0].name.as_str(), h.ball.post_hit_speed),
            ("Me", 56.9)
        );

        let goal = r#"{"Event":"GoalScored","Data":{"MatchGuid":"","GoalSpeed":87.5,"Scorer":{"Name":"Me","Shortcut":1,"TeamNum":0}}}"#;
        let GameEvent::GoalScored(g) = decode(goal).unwrap() else {
            panic!()
        };
        assert_eq!(g.goal_speed, 87.5);

        let feed = r#"{"Event":"StatfeedEvent","Data":{"MatchGuid":"G","EventName":"EpicSave","Type":"Epic Save","MainTarget":{"Name":"me","Shortcut":6,"TeamNum":1}}}"#;
        let GameEvent::Statfeed(s) = decode(feed).unwrap() else {
            panic!()
        };
        assert_eq!(
            (s.event_name.as_str(), s.main_target.unwrap().team_num),
            ("EpicSave", 1)
        );
    }

    #[test]
    fn decodes_lifecycle_events() {
        let created = r#"{"Event":"MatchCreated","Data":{"MatchGuid":"ABC"}}"#;
        assert_eq!(
            decode(created).unwrap(),
            GameEvent::MatchCreated {
                match_guid: "ABC".into()
            }
        );
        let round = r#"{"Event":"RoundStarted","Data":{"MatchGuid":""}}"#;
        assert_eq!(decode(round).unwrap(), GameEvent::RoundStarted);
        let unknown = r#"{"Event":"PodiumStart","Data":{"MatchGuid":"G"}}"#;
        assert_eq!(
            decode(unknown).unwrap(),
            GameEvent::Other("PodiumStart".into())
        );
    }

    #[test]
    fn rejects_bad_messages() {
        assert!(matches!(
            decode(r#"{"Data":{}}"#),
            Err(DecodeError::MissingEvent)
        ));
        assert!(matches!(decode("{oops}"), Err(DecodeError::Json(_))));
    }

    #[test]
    fn match_end_without_a_winner_and_pauses_decode() {
        let ended = r#"{"Event":"MatchEnded","Data":{"MatchGuid":"G"}}"#;
        assert_eq!(
            decode(ended).unwrap(),
            GameEvent::MatchEnded {
                winner_team_num: None
            }
        );
        let paused = r#"{"Event":"MatchPaused","Data":{"MatchGuid":""}}"#;
        assert_eq!(decode(paused).unwrap(), GameEvent::MatchPaused);
        let resumed = r#"{"Event":"MatchUnpaused","Data":{"MatchGuid":""}}"#;
        assert_eq!(decode(resumed).unwrap(), GameEvent::MatchUnpaused);
    }

    #[test]
    fn decodes_hit_location_and_demolition_victim() {
        let hit = r#"{"Event":"BallHit","Data":{"MatchGuid":"G","Players":[{"Name":"me","Shortcut":1,"TeamNum":0}],"Ball":{"PreHitSpeed":0,"PostHitSpeed":45.5,"Location":{"X":-399.5,"Y":396.75,"Z":115.5}}}}"#;
        let GameEvent::BallHit(h) = decode(hit).unwrap() else {
            panic!("wrong variant")
        };
        assert_eq!(
            h.ball.location,
            Some(Vec3 {
                x: -399.5,
                y: 396.75,
                z: 115.5
            })
        );
        let demo = r#"{"Event":"StatfeedEvent","Data":{"MatchGuid":"G","EventName":"Demolish","Type":"Demolition","MainTarget":{"Name":"them","Shortcut":1,"TeamNum":0},"SecondaryTarget":{"Name":"me","Shortcut":5,"TeamNum":1}}}"#;
        let GameEvent::Statfeed(f) = decode(demo).unwrap() else {
            panic!("wrong variant")
        };
        assert_eq!(f.secondary_target.map(|p| p.name).as_deref(), Some("me"));
    }

    #[test]
    fn the_last_touch_team_ignores_nobody() {
        let GameEvent::UpdateState(u) = decode(UPDATE).unwrap() else {
            panic!("wrong variant")
        };
        assert_eq!(u.game.last_touch_team(), None, "255 = no touch yet");
        let touched = UPDATE.replace(r#""TeamNum":255"#, r#""TeamNum":1"#);
        let GameEvent::UpdateState(u) = decode(&touched).unwrap() else {
            panic!("wrong variant")
        };
        assert_eq!(u.game.last_touch_team(), Some(1));
    }
}
