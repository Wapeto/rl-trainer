//! "Import past games" (v0.3.0 spec §2.2): the newest replays of games that were never recorded
//! live become sessions of their own, so a new install starts with history.

use std::panic::AssertUnwindSafe;

use chrono::{Local, NaiveDate, TimeZone};
use rl_core::ingest::now_ms;
use rl_core::store::past_games::{self, PastGame};
use rl_core::store::replays::{self, ReplayFileRecord, ReplayFileStatus, ReplayPlayer};
use rl_core::store::{profile_primary_id, EditError};

use super::extract::{self, Extracted, PlayerKey, PARSER_VERSION};
use super::poller::{self, FileInfo};
use crate::app::{App, AppError};
use crate::settings;

/// How many past games the first start and the Settings button bring in.
pub const PAST_GAMES_LIMIT: usize = 20;
/// A replay with no `TotalSecondsPlayed` is taken to be a regulation game.
const REGULATION_SECONDS: f64 = 300.0;

enum Read {
    /// Already in the log, or not a replay: the folder poller handles it.
    Skip,
    /// Its game is already in the log (recorded live or by another import).
    Known(String),
    NotMine(String),
    Failed(Option<String>, String),
    Game(String, Box<Extracted>),
}

impl App {
    /// Brings in up to `limit` past games, newest first; returns how many were added.
    pub async fn import_past_games(&self, limit: usize) -> Result<usize, AppError> {
        let me =
            profile_primary_id(&*self.read_db()?)?.and_then(|p| PlayerKey::from_primary_id(&p));
        let Some(me) = me else { return Ok(0) };
        let dirs = self.replay_dirs()?;
        let settled = past_games::settled_files(&*self.read_db()?)?;
        let mut files = tokio::task::spawn_blocking(move || poller::list(&dirs))
            .await
            .map_err(|e| AppError::Join(e.to_string()))?;
        files.sort_by_key(|f| std::cmp::Reverse(f.modified_at));
        let mut added = 0;
        for file in files {
            if added >= limit {
                break;
            }
            if settled.contains(file.path.to_string_lossy().as_ref()) {
                continue;
            }
            let path = file.path.display().to_string();
            match self.import_past_game(file, &me).await {
                Ok(true) => added += 1,
                Ok(false) => {}
                Err(err) => tracing::error!(%path, error = %err, "past game import failed"),
            }
        }
        Ok(added)
    }

    /// Once (`backfill_done`), as soon as the player is known. The profile only appears with the
    /// first live match, so the folder poller asks again on every tick. Games already in the
    /// log or deleted from it are skipped by GUID.
    pub(crate) async fn first_start_backfill(&self) -> Result<usize, AppError> {
        let due = {
            let conn = self.read_db()?;
            !settings::backfill_done(&conn)? && profile_primary_id(&conn)?.is_some()
        };
        if !due {
            return Ok(0);
        }
        settings::set_backfill_done(&*self.write_db()?)?;
        self.import_past_games(PAST_GAMES_LIMIT).await
    }

    /// Runs the import when it is due, logging the outcome.
    pub(crate) async fn run_first_backfill(&self) {
        match self.first_start_backfill().await {
            Ok(0) => {}
            Ok(n) => tracing::info!(games = n, "past games imported at the first start"),
            Err(err) => tracing::error!(error = %err, "first-start import failed"),
        }
    }

    async fn import_past_game(&self, file: FileInfo, me: &PlayerKey) -> Result<bool, AppError> {
        let read = self.read_past_game(&file, me).await?;
        let (guid, found) = match read {
            Read::Skip => return Ok(false),
            Read::Known(g) => {
                return self
                    .record_only(file, Some(g), ReplayFileStatus::Imported, None)
                    .await
            }
            Read::NotMine(g) => {
                return self
                    .record_only(file, Some(g), ReplayFileStatus::NotMine, None)
                    .await
            }
            Read::Failed(g, e) => {
                return self
                    .record_only(file, g, ReplayFileStatus::Error, Some(e))
                    .await
            }
            Read::Game(guid, found) => (guid, found),
        };
        let Some(game) = past_game(&guid, &found) else {
            let why = "the replay header has no final score or date".to_owned();
            return self
                .record_only(file, Some(guid), ReplayFileStatus::Error, Some(why))
                .await;
        };
        let (day_start, day_end) = local_day(game.started_at);
        let rec = file_record(&file, Some(guid.clone()), ReplayFileStatus::Imported, None);
        let engine = self.engine().await?;
        let added = engine
            .run(move |pipeline| {
                pipeline.with_conn(now_ms(), move |conn| -> Result<bool, EditError> {
                    let now = now_ms();
                    let tx = conn.unchecked_transaction()?;
                    let Some(id) = past_games::insert_past_game(&tx, &game, day_start, day_end)?
                    else {
                        return Ok(false);
                    };
                    replays::record_file(&tx, &rec, now)?;
                    replays::save_stats(&tx, id, &found.mine, PARSER_VERSION, now)?;
                    replays::save_players(&tx, id, &found.players)?;
                    tx.commit()?;
                    Ok(true)
                })
            })
            .await?;
        if added {
            engine.replays_imported().await?;
        } else {
            // Another import took the GUID first, or the user deleted the match.
            return self
                .record_only(file, Some(guid), ReplayFileStatus::Imported, None)
                .await;
        }
        Ok(added)
    }

    async fn read_past_game(&self, file: &FileInfo, me: &PlayerKey) -> Result<Read, AppError> {
        let path = file.path.clone();
        let header = tokio::task::spawn_blocking(move || {
            let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
            let guid = extract::read_guid(&bytes).map_err(|e| e.to_string())?;
            Ok::<_, String>((bytes, guid))
        })
        .await
        .map_err(|e| AppError::Join(e.to_string()))?;
        let (bytes, guid) = match header {
            Err(error) => return Ok(Read::Failed(None, error)),
            Ok((_, None)) => return Ok(Read::Skip),
            Ok((bytes, Some(guid))) => (bytes, guid),
        };
        let (settled, in_log) = {
            let conn = self.read_db()?;
            (
                past_games::game_settled(&conn, &guid)?,
                replays::match_id_by_guid(&conn, &guid)?.is_some(),
            )
        };
        if settled {
            return Ok(Read::Known(guid));
        }
        if in_log {
            return Ok(Read::Skip);
        }
        let me = me.clone();
        let parsed = tokio::task::spawn_blocking(move || {
            std::panic::catch_unwind(AssertUnwindSafe(|| extract::extract(&bytes, &me)))
        })
        .await
        .map_err(|e| AppError::Join(e.to_string()))?;
        Ok(match parsed {
            Ok(Ok(Some(found))) if !is_online(&found.game) => Read::NotMine(guid),
            Ok(Ok(Some(found))) => Read::Game(guid, Box::new(found)),
            Ok(Ok(None)) => Read::NotMine(guid),
            Ok(Err(err)) => Read::Failed(Some(guid), err.to_string()),
            Err(_) => Read::Failed(Some(guid), "the replay parser crashed on this file".into()),
        })
    }

    /// Remembers a file that gave no game, so the next import does not read it again.
    async fn record_only(
        &self,
        file: FileInfo,
        guid: Option<String>,
        status: ReplayFileStatus,
        error: Option<String>,
    ) -> Result<bool, AppError> {
        let rec = file_record(&file, guid, status, error);
        self.engine()
            .await?
            .run(move |pipeline| {
                pipeline.with_conn(now_ms(), move |conn| -> Result<(), EditError> {
                    Ok(replays::record_file(conn, &rec, now_ms())?)
                })
            })
            .await?;
        Ok(false)
    }
}

fn file_record(
    file: &FileInfo,
    match_guid: Option<String>,
    status: ReplayFileStatus,
    error: Option<String>,
) -> ReplayFileRecord {
    ReplayFileRecord {
        path: file.path.to_string_lossy().into_owned(),
        modified_at: file.modified_at,
        size: file.size,
        match_guid,
        status,
        error,
    }
}

/// Private matches, exhibitions and the like are not part of the match history.
fn is_online(game: &extract::GameHeader) -> bool {
    game.match_type.as_deref() == Some("Online")
}

/// The header `Date` is when the replay was saved: the end of the game.
fn past_game(guid: &str, found: &Extracted) -> Option<PastGame> {
    let outcome = found.outcome?;
    let ended_at = found.game.ended_at_ms?;
    let seconds = found.game.seconds_played.unwrap_or(REGULATION_SECONDS);
    let me = found.players.iter().find(|p| p.is_me)?;
    let stat = |v: Option<f64>| v.map_or(0, |x| x.round() as i64);
    Some(PastGame {
        match_guid: guid.to_owned(),
        started_at: ended_at - (seconds * 1000.0) as i64,
        ended_at,
        team_size: found.game.team_size,
        arena: found.game.arena.clone().unwrap_or_default(),
        outcome,
        score: stat(me.stats.score),
        goals: stat(me.stats.goals),
        assists: stat(me.stats.assists),
        saves: stat(me.stats.saves),
        shots: stat(me.stats.shots),
        scoreboard_json: scoreboard_json(&found.players),
    })
}

/// The same rows the live recorder writes (`ScoreboardRow`).
fn scoreboard_json(players: &[ReplayPlayer]) -> String {
    let rows: Vec<_> = players
        .iter()
        .map(|p| {
            let stat = |v: Option<f64>| v.map_or(0, |x| x.round() as i64);
            serde_json::json!({
                "name": p.name,
                "team_num": p.team,
                "score": stat(p.stats.score),
                "goals": stat(p.stats.goals),
                "assists": stat(p.stats.assists),
                "saves": stat(p.stats.saves),
                "shots": stat(p.stats.shots),
                "demos": stat(p.stats.demos_inflicted),
                "me": p.is_me,
            })
        })
        .collect();
    serde_json::to_string(&rows).unwrap_or_else(|_| "[]".to_owned())
}

/// Local midnight before and after `ms`.
fn local_day(ms: i64) -> (i64, i64) {
    let day = Local
        .timestamp_millis_opt(ms)
        .single()
        .map(|t| t.date_naive())
        .unwrap_or_default();
    let midnight = |d: NaiveDate| {
        d.and_hms_opt(0, 0, 0)
            .and_then(|t| Local.from_local_datetime(&t).earliest())
            .map_or(ms, |t| t.timestamp_millis())
    };
    (midnight(day), midnight(day.succ_opt().unwrap_or(day)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_online_games_are_past_games() {
        let header = |t: Option<&str>| extract::GameHeader {
            match_type: t.map(str::to_owned),
            ..Default::default()
        };
        assert!(is_online(&header(Some("Online"))));
        assert!(!is_online(&header(Some("Private"))));
        assert!(!is_online(&header(None)));
    }

    #[test]
    fn a_local_day_holds_its_moment() {
        let now = now_ms();
        let (start, end) = local_day(now);
        assert!(start <= now && now < end);
        assert!(
            (end - start - 86_400_000).abs() <= 3_600_000,
            "a day, give or take DST"
        );
    }
}
