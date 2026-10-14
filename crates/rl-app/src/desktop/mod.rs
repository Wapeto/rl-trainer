//! The desktop window's commands. One dispatcher serves the Tauri `desktop` command and the
//! loopback HTTP shim (`rl-replay serve --desktop-port`: the Vite dev server and Playwright).

mod coach;
pub mod http;
mod progress;

use rl_core::store::edits::MmrInput;
use rl_core::store::packs::{PackInput, PackSort};
use rl_core::store::sessions::Neighbour;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::app::App;
use crate::settings::SettingsPatch;

/// The replay folders and what was found in them (Settings).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ReplayStatus {
    /// The folders being watched (none when the game's folder was not found).
    pub folders: Vec<String>,
    pub counts: rl_core::store::replays::ReplayCounts,
}

/// One match's replay page: every player, the winners' benchmark in this mode, and what stood out.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct MatchReplay {
    pub players: Vec<rl_core::store::replays::ReplayPlayer>,
    pub benchmark: rl_core::coach::benchmark::Benchmark,
    pub insights: Vec<rl_core::coach::insights::Insight>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct About {
    pub version: String,
    /// Where the database, backups and logs live.
    pub data_dir: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NoArgs {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Id {
    pub(super) id: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionId {
    session_id: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BlockId {
    block_id: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sessions {
    search: String,
    offset: i64,
    limit: i64,
    /// Only sessions with a ranked match whose replay is still to save.
    #[serde(default)]
    to_save: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BlockPack {
    block_id: i64,
    pack_id: Option<i64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Review {
    session_id: i64,
    mmr: Vec<MmrInput>,
    notes: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Merge {
    session_id: i64,
    with: Neighbour,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Split {
    session_id: i64,
    after_block_id: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Packs {
    sort: PackSort,
    search: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewPack {
    pack: PackInput,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EditPack {
    id: i64,
    pack: PackInput,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Hidden {
    id: i64,
    hidden: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Patch {
    patch: SettingsPatch,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Profile {
    primary_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BackupName {
    name: String,
}

fn parse<T: DeserializeOwned>(args: &Value) -> Result<T, String> {
    let args = if args.is_null() {
        Value::Object(serde_json::Map::new())
    } else {
        args.clone()
    };
    serde_json::from_value(args).map_err(|e| format!("bad arguments: {e}"))
}

fn report_args(args: &Value) -> Result<crate::report::Report, String> {
    parse::<crate::report::ReportArgs>(args)?
        .try_into()
        .map_err(text)
}

fn reply<T: Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

fn text(err: impl std::fmt::Display) -> String {
    err.to_string()
}

/// Runs one desktop command. `args` is a JSON object (`null` when there are none).
pub async fn dispatch(app: &App, command: &str, args: Value) -> Result<Value, String> {
    if let Some(result) = coach::coach(app, command, &args) {
        return result;
    }
    if let Some(result) = progress::progress(app, command, &args) {
        return result;
    }
    if let Some(result) = library(app, command, &args).await {
        return result;
    }
    if let Some(result) = system(app, command, &args).await {
        return result;
    }
    Err(format!("unknown command: {command}"))
}

/// Sessions and packs.
async fn library(app: &App, command: &str, args: &Value) -> Option<Result<Value, String>> {
    let result = match command {
        "sessions" => match parse::<Sessions>(args) {
            Ok(a) if a.to_save => app.session_cards_to_save().map_err(text).and_then(reply),
            Ok(a) => app
                .session_cards(&a.search, a.offset, a.limit)
                .map_err(text)
                .and_then(reply),
            Err(e) => Err(e),
        },
        "review_count" => {
            parse::<NoArgs>(args).and_then(|_| app.review_count().map_err(text).and_then(reply))
        }
        "session" => {
            parse::<Id>(args).and_then(|a| app.session_detail(a.id).map_err(text).and_then(reply))
        }
        "set_block_pack" => match parse::<BlockPack>(args) {
            Ok(a) => app
                .set_block_pack(a.block_id, a.pack_id)
                .await
                .map_err(text)
                .and_then(reply),
            Err(e) => Err(e),
        },
        "edit_review" => match parse::<Review>(args) {
            Ok(a) => app
                .edit_review(a.session_id, a.mmr, a.notes)
                .await
                .map_err(text)
                .and_then(reply),
            Err(e) => Err(e),
        },
        "merge_sessions" => match parse::<Merge>(args) {
            Ok(a) => app
                .merge_sessions(a.session_id, a.with)
                .await
                .map_err(text)
                .and_then(reply),
            Err(e) => Err(e),
        },
        "split_session" => match parse::<Split>(args) {
            Ok(a) => app
                .split_session(a.session_id, a.after_block_id)
                .await
                .map_err(text)
                .and_then(reply),
            Err(e) => Err(e),
        },
        "delete_session" => match parse::<SessionId>(args) {
            Ok(a) => app
                .delete_session(a.session_id)
                .await
                .map_err(text)
                .and_then(reply),
            Err(e) => Err(e),
        },
        "delete_activity" => match parse::<BlockId>(args) {
            Ok(a) => app
                .delete_activity(a.block_id)
                .await
                .map_err(text)
                .and_then(reply),
            Err(e) => Err(e),
        },
        "packs" => parse::<Packs>(args).and_then(|a| {
            app.pack_list(a.sort, &a.search)
                .map_err(text)
                .and_then(reply)
        }),
        "pack" => {
            parse::<Id>(args).and_then(|a| app.pack_detail(a.id).map_err(text).and_then(reply))
        }
        "add_pack" => match parse::<NewPack>(args) {
            Ok(a) => app.add_pack(a.pack).await.map_err(text).and_then(reply),
            Err(e) => Err(e),
        },
        "update_pack" => match parse::<EditPack>(args) {
            Ok(a) => app
                .update_pack(a.id, a.pack)
                .await
                .map_err(text)
                .and_then(reply),
            Err(e) => Err(e),
        },
        "set_pack_hidden" => match parse::<Hidden>(args) {
            Ok(a) => app
                .set_pack_hidden(a.id, a.hidden)
                .await
                .map_err(text)
                .and_then(reply),
            Err(e) => Err(e),
        },
        _ => return None,
    };
    Some(result)
}

/// The app itself: status, settings, phone, Stats API, backups.
async fn system(app: &App, command: &str, args: &Value) -> Option<Result<Value, String>> {
    const NO_ARGS: [&str; 15] = [
        "snapshot",
        "startup_report",
        "about",
        "settings",
        "profiles",
        "pairing",
        "enable_phone",
        "disable_phone",
        "regenerate_token",
        "fix_stats_api",
        "recheck_stats_api",
        "backups",
        "open_data_folder",
        "open_releases",
        "report_info",
    ];
    if NO_ARGS.contains(&command) {
        // Check the arguments before running anything.
        if let Err(e) = parse::<NoArgs>(args) {
            return Some(Err(e));
        }
        return system_no_args(app, command).await;
    }
    let result = match command {
        "update_settings" => match parse::<Patch>(args) {
            Ok(a) => app
                .update_settings(&a.patch)
                .await
                .map_err(text)
                .and_then(reply),
            Err(e) => Err(e),
        },
        "set_active_profile" => parse::<Profile>(args).and_then(|a| {
            app.set_active_profile(&a.primary_id)
                .map_err(text)
                .and_then(reply)
        }),
        "send_report" => match report_args(args) {
            Ok(report) => app.send_report(report).await.map_err(text).and_then(reply),
            Err(e) => Err(e),
        },
        "save_report" => report_args(args).and_then(|report| {
            app.save_report(&report)
                .map_err(text)
                .and_then(|dir| reply(dir.display().to_string()))
        }),
        "restore_backup" => match parse::<BackupName>(args) {
            Ok(a) => app
                .restore_backup(&a.name)
                .await
                .map_err(text)
                .and_then(reply),
            Err(e) => Err(e),
        },
        _ => return None,
    };
    Some(result)
}

async fn system_no_args(app: &App, command: &str) -> Option<Result<Value, String>> {
    let result = match command {
        "snapshot" => reply(app.snapshot()),
        "startup_report" => reply(app.startup_report()),
        "about" => reply(About {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            data_dir: app.paths().root.display().to_string(),
        }),
        "settings" => app.settings().map_err(text).and_then(reply),
        "profiles" => app.profiles().map_err(text).and_then(reply),
        "pairing" => reply(app.pairing().await),
        "enable_phone" => app.enable_phone().await.map_err(text).and_then(reply),
        "disable_phone" => app.disable_phone().await.map_err(text).and_then(reply),
        "regenerate_token" => app.regenerate_token().await.map_err(text).and_then(reply),
        "fix_stats_api" => app.fix_stats_api().await.map_err(text).and_then(reply),
        "recheck_stats_api" => reply(app.recheck_stats_api().await),
        "backups" => app.list_backups().map_err(text).and_then(reply),
        "report_info" => reply(app.report_info()),
        "open_data_folder" => app.open_data_folder().map_err(text).and_then(reply),
        "open_releases" => app.open_releases().map_err(text).and_then(reply),
        _ => return None,
    };
    Some(result)
}
