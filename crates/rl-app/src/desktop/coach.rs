//! The coach's commands: the session review and the replays to save.

use serde_json::Value;

use super::{parse, reply, text, Id, NoArgs};
use crate::app::App;

pub(super) fn coach(app: &App, command: &str, args: &Value) -> Option<Result<Value, String>> {
    let result =
        match command {
            "session_review" => parse::<Id>(args)
                .and_then(|a| app.session_review(a.id).map_err(text).and_then(reply)),
            "replays_to_save" => parse::<NoArgs>(args)
                .and_then(|_| app.replays_to_save().map_err(text).and_then(reply)),
            "match_replay" => {
                parse::<Id>(args).and_then(|a| app.match_replay(a.id).map_err(text).and_then(reply))
            }
            "replay_status" => parse::<NoArgs>(args)
                .and_then(|_| app.replay_status().map_err(text).and_then(reply)),
            _ => return None,
        };
    Some(result)
}
