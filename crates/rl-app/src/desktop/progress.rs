//! The Progress page's commands.

use rl_core::store::progress::ProgressRange;
use serde::Deserialize;
use serde_json::Value;

use super::{parse, reply, text};
use crate::app::App;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Range {
    range: ProgressRange,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mode {
    team_size: u8,
    range: ProgressRange,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TeamSize {
    team_size: u8,
}

fn check_team_size(team_size: u8) -> Result<(), String> {
    if (1..=3).contains(&team_size) {
        Ok(())
    } else {
        Err("the team size must be 1, 2 or 3".to_owned())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pack {
    pack_id: i64,
    range: ProgressRange,
}

pub(super) fn progress(app: &App, command: &str, args: &Value) -> Option<Result<Value, String>> {
    let result = match command {
        "progress_list" => parse::<Range>(args)
            .and_then(|a| app.progress_list(a.range).map_err(text).and_then(reply)),
        "progress_overview" => parse::<Range>(args)
            .and_then(|a| app.progress_overview(a.range).map_err(text).and_then(reply)),
        "progress_mode" => parse::<Mode>(args).and_then(|a| {
            check_team_size(a.team_size)?;
            app.progress_mode(a.team_size, a.range)
                .map_err(text)
                .and_then(reply)
        }),
        "mode_benchmark" => parse::<TeamSize>(args).and_then(|a| {
            check_team_size(a.team_size)?;
            app.mode_benchmark(a.team_size)
                .map_err(text)
                .and_then(reply)
        }),
        "progress_pack" => parse::<Pack>(args).and_then(|a| {
            app.progress_pack(a.pack_id, a.range)
                .map_err(text)
                .and_then(reply)
        }),
        _ => return None,
    };
    Some(result)
}
