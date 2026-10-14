//! Read models for the CLI (and later the UI).

use rusqlite::{params, Connection, OptionalExtension};
use ts_rs::TS;

use crate::store::attempt_sql::{counted, made};

#[derive(Debug, Clone, PartialEq)]
pub struct BlockRow {
    pub id: i64,
    pub kind: String,
    pub playlist_id: Option<i32>,
    pub attempts: i64,
    pub makes: i64,
    pub match_result: Option<String>,
}

impl BlockRow {
    /// Makes ÷ non-skipped attempts; `None` when there were no attempts.
    pub fn make_rate(&self) -> Option<f64> {
        (self.attempts > 0).then(|| self.makes as f64 / self.attempts as f64)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionRow {
    pub id: i64,
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,
    pub blocks: Vec<BlockRow>,
}

/// All sessions, newest first, each with its blocks in order.
pub fn list_sessions(conn: &Connection) -> rusqlite::Result<Vec<SessionRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, started_at, ended_at FROM session ORDER BY started_at DESC, id DESC",
    )?;
    let sessions: Vec<(i64, i64, Option<i64>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    sessions
        .into_iter()
        .map(|(id, started_at_ms, ended_at_ms)| {
            Ok(SessionRow {
                id,
                started_at_ms,
                ended_at_ms,
                blocks: list_blocks(conn, id)?,
            })
        })
        .collect()
}

fn list_blocks(conn: &Connection, session_id: i64) -> rusqlite::Result<Vec<BlockRow>> {
    let sql = format!(
        "SELECT b.id, b.kind, b.playlist_id,
                (SELECT COUNT(*) FROM training_attempt a WHERE a.block_id = b.id AND {c}),
                (SELECT COUNT(*) FROM training_attempt a
                   WHERE a.block_id = b.id AND {c} AND {m}),
                (SELECT m.result FROM match_record m WHERE m.block_id = b.id)
         FROM block b WHERE b.session_id = ?1 ORDER BY b.started_at, b.id",
        c = counted("a"),
        m = made("a")
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![session_id], |r| {
        Ok(BlockRow {
            id: r.get(0)?,
            kind: r.get(1)?,
            playlist_id: r.get(2)?,
            attempts: r.get(3)?,
            makes: r.get(4)?,
            match_result: r.get(5)?,
        })
    })?;
    rows.collect()
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, TS)]
pub struct ProfileRow {
    pub primary_id: String,
    pub display_name: String,
    pub is_active: bool,
}

/// The active profile ("me"), if one has been detected or chosen.
pub fn profile_primary_id(conn: &Connection) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT primary_id FROM profile WHERE is_active = 1",
        [],
        |r| r.get(0),
    )
    .optional()
}

pub fn list_profiles(conn: &Connection) -> rusqlite::Result<Vec<ProfileRow>> {
    let mut stmt = conn.prepare(
        "SELECT primary_id, display_name, is_active FROM profile ORDER BY created_at, primary_id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ProfileRow {
            primary_id: r.get(0)?,
            display_name: r.get(1)?,
            is_active: r.get(2)?,
        })
    })?;
    rows.collect()
}

/// No profile is active, so the recorder detects "me" again on the next start.
pub fn clear_active_profile(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("UPDATE profile SET is_active = 0 WHERE is_active = 1", [])?;
    Ok(())
}

/// Makes `primary_id` the only active profile. Returns false when no such profile exists.
pub fn set_active_profile(
    conn: &Connection,
    primary_id: &str,
) -> Result<bool, crate::store::StoreError> {
    let tx = conn.unchecked_transaction()?;
    let exists: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM profile WHERE primary_id = ?1)",
        params![primary_id],
        |r| r.get(0),
    )?;
    if !exists {
        return Ok(false);
    }
    tx.execute("UPDATE profile SET is_active = 0 WHERE is_active = 1", [])?;
    tx.execute(
        "UPDATE profile SET is_active = 1 WHERE primary_id = ?1",
        params![primary_id],
    )?;
    tx.commit()?;
    Ok(true)
}
