//! Importing one replay file (spec §4.2): header first, then the stats pass on a blocking
//! thread, then one write through the engine.

use std::panic::AssertUnwindSafe;

use rl_core::ingest::now_ms;
use rl_core::store::past_games;
use rl_core::store::replays::{self, ReplayFileRecord, ReplayFileStatus};
use rl_core::store::{profile_primary_id, EditError};

use super::extract::{self, Extracted, PlayerKey, PARSER_VERSION};
use super::poller::FileInfo;
use crate::app::{App, AppError};

enum Outcome {
    Unmatched(Option<String>),
    NotMine(String),
    Stats(String, i64, Box<Extracted>),
    Failed(Option<String>, String),
}

impl App {
    /// Reads and records one replay file; returns what became of it.
    pub async fn import_replay(&self, file: FileInfo) -> Result<ReplayFileStatus, AppError> {
        let me =
            profile_primary_id(&*self.read_db()?)?.and_then(|p| PlayerKey::from_primary_id(&p));
        let path = file.path.clone();
        let header = tokio::task::spawn_blocking(move || {
            std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|b| {
                    extract::read_guid(&b)
                        .map(|g| (b, g))
                        .map_err(|e| e.to_string())
                })
        })
        .await
        .map_err(|e| AppError::Join(e.to_string()))?;
        let outcome = match header {
            Err(error) => Outcome::Failed(None, error),
            Ok((_, None)) => Outcome::Unmatched(None),
            Ok((bytes, Some(guid))) => self.read_stats(bytes, guid, me).await?,
        };
        self.record(file, outcome).await
    }

    async fn read_stats(
        &self,
        bytes: Vec<u8>,
        guid: String,
        me: Option<PlayerKey>,
    ) -> Result<Outcome, AppError> {
        let Some(match_id) = replays::match_id_by_guid(&*self.read_db()?, &guid)? else {
            return Ok(Outcome::Unmatched(Some(guid)));
        };
        let Some(me) = me else {
            return Ok(Outcome::NotMine(guid));
        };
        let parsed = tokio::task::spawn_blocking(move || {
            std::panic::catch_unwind(AssertUnwindSafe(|| extract::extract(&bytes, &me)))
        })
        .await
        .map_err(|e| AppError::Join(e.to_string()))?;
        Ok(match parsed {
            Ok(Ok(Some(found))) => Outcome::Stats(guid, match_id, Box::new(found)),
            Ok(Ok(None)) => Outcome::NotMine(guid),
            Ok(Err(err)) => Outcome::Failed(Some(guid), err.to_string()),
            Err(_) => Outcome::Failed(Some(guid), "the replay parser crashed on this file".into()),
        })
    }

    async fn record(&self, file: FileInfo, outcome: Outcome) -> Result<ReplayFileStatus, AppError> {
        let (guid, status, error, stats) = match outcome {
            Outcome::Unmatched(g) => (g, ReplayFileStatus::Unmatched, None, None),
            Outcome::NotMine(g) => (Some(g), ReplayFileStatus::NotMine, None, None),
            Outcome::Stats(g, id, x) => (Some(g), ReplayFileStatus::Imported, None, Some((id, x))),
            Outcome::Failed(g, e) => (g, ReplayFileStatus::Error, Some(e), None),
        };
        let rec = ReplayFileRecord {
            path: file.path.to_string_lossy().into_owned(),
            modified_at: file.modified_at,
            size: file.size,
            match_guid: guid,
            status,
            error,
        };
        let imported = stats.is_some();
        let engine = self.engine().await?;
        engine
            .run(move |pipeline| {
                pipeline.with_conn(now_ms(), move |conn| -> Result<(), EditError> {
                    let now = now_ms();
                    let tx = conn.unchecked_transaction()?;
                    replays::record_file(&tx, &rec, now)?;
                    if let Some((id, x)) = &stats {
                        replays::save_stats(&tx, *id, &x.mine, PARSER_VERSION, now)?;
                        replays::save_players(&tx, *id, &x.players)?;
                        if let Some(outcome) = x.outcome {
                            past_games::apply_outcome(&tx, *id, outcome)?;
                        }
                    }
                    tx.commit()?;
                    Ok(())
                })
            })
            .await?;
        if imported {
            engine.replays_imported().await?;
        }
        Ok(status)
    }
}
