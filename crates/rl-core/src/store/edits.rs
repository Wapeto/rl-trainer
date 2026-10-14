//! Validated user edits: pack on closed blocks, skip flags, MMR entries, session notes.

use std::ops::RangeInclusive;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::store::attempt_sql::{counted, made};
use crate::store::scoring::{is_goalie, rescore_block};
use crate::store::StoreError;

pub const MAX_NOTES_CHARS: usize = 2_000;
pub const MMR_RANGE: RangeInclusive<i32> = 0..=3_000;
pub const TEAM_SIZES: RangeInclusive<u8> = 1..=3;

#[derive(Debug, Error)]
pub enum EditError {
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<rusqlite::Error> for EditError {
    fn from(err: rusqlite::Error) -> Self {
        Self::Store(StoreError::Sqlite(err))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct PackRow {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub creator: String,
    pub tags: Vec<String>,
    pub shot_count: Option<i64>,
    pub target_rate: f64,
    /// Hidden packs keep their history but leave the phone's picker and suggestions.
    pub hidden: bool,
    pub notes: String,
    /// Goalie pack: a shot is made unless the ball goes in.
    pub goalie: bool,
}

const PACK_COLUMNS: &str =
    "id, code, name, creator, tags, shot_count, target_rate, hidden, notes, goalie";

fn pack_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<PackRow> {
    let tags: String = r.get(4)?;
    Ok(PackRow {
        id: r.get(0)?,
        code: r.get(1)?,
        name: r.get(2)?,
        creator: r.get(3)?,
        // Tags are written by this app as a JSON array; anything else reads as no tags.
        tags: serde_json::from_str(&tags).unwrap_or_default(),
        shot_count: r.get(5)?,
        target_rate: r.get(6)?,
        hidden: r.get(7)?,
        notes: r.get(8)?,
        goalie: r.get(9)?,
    })
}

pub fn list_packs(conn: &Connection) -> rusqlite::Result<Vec<PackRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {PACK_COLUMNS} FROM pack ORDER BY name COLLATE NOCASE, id"
    ))?;
    let rows = stmt.query_map([], pack_row)?;
    rows.collect()
}

pub fn pack(conn: &Connection, id: i64) -> rusqlite::Result<Option<PackRow>> {
    conn.query_row(
        &format!("SELECT {PACK_COLUMNS} FROM pack WHERE id = ?1"),
        params![id],
        pack_row,
    )
    .optional()
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct PackStats {
    pub pack: PackRow,
    /// All-time attempts on the pack, skipped ones excluded.
    pub attempts: i64,
    pub makes: i64,
    pub make_rate: Option<f64>,
    pub last_trained_at: Option<i64>,
}

/// Visible packs with their all-time stats (the phone's picker).
pub fn pack_stats(conn: &Connection) -> rusqlite::Result<Vec<PackStats>> {
    all_pack_stats(conn, false)
}

/// Every pack with its all-time stats, by name; hidden ones only when asked.
pub(crate) fn all_pack_stats(
    conn: &Connection,
    include_hidden: bool,
) -> rusqlite::Result<Vec<PackStats>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT p.id, p.code, p.name, p.creator, p.tags, p.shot_count, p.target_rate, p.hidden,
                p.notes, COALESCE(s.attempts, 0), COALESCE(s.makes, 0), s.last_at
         FROM pack p
         LEFT JOIN (SELECT b.pack_id, COUNT(*) AS attempts, SUM({made}) AS makes,
                           MAX(a.started_at) AS last_at
                    FROM training_attempt a JOIN block b ON b.id = a.block_id
                    WHERE {counted} AND b.pack_id IS NOT NULL
                    GROUP BY b.pack_id) s ON s.pack_id = p.id
         WHERE ?1 OR p.hidden = 0
         ORDER BY p.name COLLATE NOCASE, p.id",
        made = made("a"),
        counted = counted("a")
    ))?;
    let rows = stmt.query_map(params![include_hidden], |r| {
        let attempts: i64 = r.get(9)?;
        let makes: i64 = r.get(10)?;
        Ok(PackStats {
            pack: pack_row(r)?,
            attempts,
            makes,
            make_rate: (attempts > 0).then(|| makes as f64 / attempts as f64),
            last_trained_at: r.get(11)?,
        })
    })?;
    rows.collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct MmrInput {
    pub team_size: u8,
    pub mmr: i32,
}

/// Saves the end-of-session review in one transaction: MMR per ranked team size, the note and
/// the reviewed mark. An empty review (no MMR, no note) only marks the session (Skip).
pub fn review_session(
    conn: &Connection,
    session_id: i64,
    mmr: &[MmrInput],
    notes: Option<&str>,
    at_ms: i64,
) -> Result<(), EditError> {
    validate_mmr_list(mmr)?;
    let notes = notes.map(clean_notes).transpose()?;
    // A review covers what has ended so far: an open session is reviewed in the menus, and
    // again once something new has ended.
    let state: Option<(bool, bool)> = conn
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM block b WHERE b.session_id = s.id AND b.ended_at IS NULL),
                    s.reviewed_at IS NOT NULL AND NOT EXISTS (
                        SELECT 1 FROM block b WHERE b.session_id = s.id AND b.ended_at > s.reviewed_at)
             FROM session s WHERE s.id = ?1",
            params![session_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    match state {
        None => return Err(EditError::NotFound("session")),
        Some((true, _)) => {
            return Err(EditError::Conflict(
                "finish the current activity first".into(),
            ))
        }
        Some((_, true)) => {
            return Err(EditError::Conflict(
                "this session has already been reviewed".into(),
            ))
        }
        Some(_) => {}
    }
    let tx = conn.unchecked_transaction()?;
    for entry in mmr {
        tx.execute(
            "INSERT INTO mmr_entry (team_size, mmr, recorded_at, session_id) VALUES (?1, ?2, ?3, ?4)",
            params![entry.team_size, entry.mmr, at_ms, session_id],
        )?;
    }
    if let Some(notes) = &notes {
        tx.execute(
            "UPDATE session SET notes = ?1 WHERE id = ?2",
            params![notes, session_id],
        )?;
    }
    tx.execute(
        "UPDATE session SET reviewed_at = ?1 WHERE id = ?2",
        params![at_ms, session_id],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn validate_mmr(team_size: u8, mmr: i32) -> Result<(), EditError> {
    if !TEAM_SIZES.contains(&team_size) {
        return Err(EditError::Invalid("team size must be 1, 2 or 3".into()));
    }
    if !MMR_RANGE.contains(&mmr) {
        return Err(EditError::Invalid("MMR must be between 0 and 3000".into()));
    }
    Ok(())
}

/// Every entry valid, each team size at most once.
pub(crate) fn validate_mmr_list(mmr: &[MmrInput]) -> Result<(), EditError> {
    for (i, entry) in mmr.iter().enumerate() {
        validate_mmr(entry.team_size, entry.mmr)?;
        if mmr[..i].iter().any(|e| e.team_size == entry.team_size) {
            return Err(EditError::Invalid(format!(
                "MMR for {0}v{0} was given twice",
                entry.team_size
            )));
        }
    }
    Ok(())
}

fn session_exists(conn: &Connection, id: i64) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM session WHERE id = ?1)",
        params![id],
        |r| r.get(0),
    )
}

pub fn add_mmr(
    conn: &Connection,
    team_size: u8,
    mmr: i32,
    session_id: Option<i64>,
    at_ms: i64,
) -> Result<i64, EditError> {
    validate_mmr(team_size, mmr)?;
    if let Some(id) = session_id {
        if !session_exists(conn, id)? {
            return Err(EditError::NotFound("session"));
        }
    }
    conn.execute(
        "INSERT INTO mmr_entry (team_size, mmr, recorded_at, session_id) VALUES (?1, ?2, ?3, ?4)",
        params![team_size, mmr, at_ms, session_id],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn clean_notes(raw: &str) -> Result<String, EditError> {
    let cleaned: String = raw
        .replace("\r\n", "\n")
        .chars()
        .filter(|c| *c == '\n' || !c.is_control())
        .collect();
    let cleaned = cleaned.trim().to_owned();
    if cleaned.chars().count() > MAX_NOTES_CHARS {
        return Err(EditError::Invalid(format!(
            "notes are limited to {MAX_NOTES_CHARS} characters"
        )));
    }
    Ok(cleaned)
}

pub fn set_session_notes(
    conn: &Connection,
    session_id: i64,
    raw: &str,
) -> Result<String, EditError> {
    let notes = clean_notes(raw)?;
    let changed = conn.execute(
        "UPDATE session SET notes = ?1 WHERE id = ?2",
        params![notes, session_id],
    )?;
    if changed == 0 {
        return Err(EditError::NotFound("session"));
    }
    Ok(notes)
}

pub fn set_attempt_skipped(
    conn: &Connection,
    attempt_id: i64,
    skipped: bool,
    open_session_id: Option<i64>,
) -> Result<(), EditError> {
    let session: Option<i64> = conn
        .query_row(
            "SELECT b.session_id FROM training_attempt a JOIN block b ON b.id = a.block_id
             WHERE a.id = ?1",
            params![attempt_id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(session) = session else {
        return Err(EditError::NotFound("attempt"));
    };
    if Some(session) != open_session_id {
        return Err(EditError::Conflict(
            "that attempt is from a session that already ended".into(),
        ));
    }
    conn.execute(
        "UPDATE training_attempt SET skipped = ?1 WHERE id = ?2",
        params![skipped, attempt_id],
    )?;
    Ok(())
}

pub fn set_closed_block_pack(
    conn: &Connection,
    block_id: i64,
    pack_id: Option<i64>,
) -> Result<(), EditError> {
    let block: Option<(String, Option<i64>)> = conn
        .query_row(
            "SELECT kind, ended_at FROM block WHERE id = ?1",
            params![block_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((kind, ended_at)) = block else {
        return Err(EditError::NotFound("activity"));
    };
    if kind != "training" {
        return Err(EditError::Invalid(
            "only training activities have a pack".into(),
        ));
    }
    if ended_at.is_none() {
        return Err(EditError::Conflict(
            "the activity is still being recorded".into(),
        ));
    }
    if let Some(id) = pack_id {
        if pack(conn, id)?.is_none() {
            return Err(EditError::NotFound("pack"));
        }
    }
    let old: Option<i64> = conn.query_row(
        "SELECT pack_id FROM block WHERE id = ?1",
        params![block_id],
        |r| r.get(0),
    )?;
    let (was_goalie, goalie) = (is_goalie(conn, old)?, is_goalie(conn, pack_id)?);
    conn.execute(
        "UPDATE block SET pack_id = ?1 WHERE id = ?2",
        params![pack_id, block_id],
    )?;
    rescore_block(conn, block_id, was_goalie, goalie)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::open_in_memory;

    fn db() -> Connection {
        let conn = open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO session (id, started_at, ended_at) VALUES (1, 0, 100), (2, 200, NULL);
             INSERT INTO block (id, session_id, kind, started_at, ended_at)
                 VALUES (10, 1, 'training', 0, 100), (11, 1, 'freeplay', 0, 100),
                        (20, 2, 'training', 200, NULL);
             INSERT INTO training_attempt (id, block_id, seq, started_at, duration_ms, outcome)
                 VALUES (100, 10, 1, 0, 3000, 'make'), (200, 20, 1, 200, 3000, 'miss');",
        )
        .unwrap();
        conn
    }

    #[test]
    fn aerial_shots_pass_is_available() {
        let packs = list_packs(&db()).unwrap();
        let pack = packs
            .iter()
            .find(|p| p.code == "C7E0-9E0B-B739-A899")
            .expect("the pack is in the list");
        assert_eq!(pack.name, "Aerial Shots - Pass");
    }

    #[test]
    fn packs_are_listed_by_name_with_tags() {
        let packs = list_packs(&db()).unwrap();
        assert_eq!(packs.len(), 11);
        assert_eq!(packs[0].name, "Aerial Off Wall");
        let walls = packs
            .iter()
            .find(|p| p.code == "9274-D4A2-967B-0D7A")
            .unwrap();
        assert_eq!(walls.tags, ["defence", "wall"]);
        assert_eq!(pack(&db(), walls.id).unwrap().unwrap().name, "Wall Clears");
        assert!(pack(&db(), 999).unwrap().is_none());
    }

    #[test]
    fn mmr_is_validated_and_linked_to_an_existing_session() {
        let conn = db();
        assert!(matches!(validate_mmr(4, 900), Err(EditError::Invalid(_))));
        assert!(matches!(validate_mmr(2, 3_001), Err(EditError::Invalid(_))));
        assert!(matches!(validate_mmr(2, -1), Err(EditError::Invalid(_))));
        assert!(matches!(
            add_mmr(&conn, 2, 900, Some(99), 5),
            Err(EditError::NotFound("session"))
        ));
        let id = add_mmr(&conn, 2, 899, Some(1), 5).unwrap();
        let row: (i64, i64, Option<i64>) = conn
            .query_row(
                "SELECT team_size, mmr, session_id FROM mmr_entry WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(row, (2, 899, Some(1)));
        add_mmr(&conn, 1, 0, None, 6).unwrap();
        add_mmr(&conn, 3, 3_000, None, 7).unwrap();
    }

    #[test]
    fn notes_are_trimmed_cleaned_and_limited() {
        assert_eq!(
            clean_notes("  good\r\nsession\u{7}\t ").unwrap(),
            "good\nsession"
        );
        let too_long = "x".repeat(MAX_NOTES_CHARS + 1);
        assert!(matches!(clean_notes(&too_long), Err(EditError::Invalid(_))));
        let conn = db();
        assert_eq!(set_session_notes(&conn, 1, " ok ").unwrap(), "ok");
        assert!(matches!(
            set_session_notes(&conn, 9, "x"),
            Err(EditError::NotFound("session"))
        ));
    }

    #[test]
    fn skip_is_limited_to_the_open_session() {
        let conn = db();
        assert!(matches!(
            set_attempt_skipped(&conn, 100, true, Some(2)),
            Err(EditError::Conflict(_))
        ));
        assert!(matches!(
            set_attempt_skipped(&conn, 999, true, Some(2)),
            Err(EditError::NotFound("attempt"))
        ));
        assert!(matches!(
            set_attempt_skipped(&conn, 200, true, None),
            Err(EditError::Conflict(_))
        ));
        set_attempt_skipped(&conn, 200, true, Some(2)).unwrap();
        set_attempt_skipped(&conn, 200, false, Some(2)).unwrap();
        let skipped: bool = conn
            .query_row(
                "SELECT skipped FROM training_attempt WHERE id = 200",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!skipped, "toggle back");
    }

    #[test]
    fn closed_training_blocks_take_a_pack() {
        let conn = db();
        set_closed_block_pack(&conn, 10, Some(3)).unwrap();
        set_closed_block_pack(&conn, 10, None).unwrap();
        assert!(matches!(
            set_closed_block_pack(&conn, 11, Some(3)),
            Err(EditError::Invalid(_))
        ));
        assert!(matches!(
            set_closed_block_pack(&conn, 20, Some(3)),
            Err(EditError::Conflict(_))
        ));
        assert!(matches!(
            set_closed_block_pack(&conn, 99, Some(3)),
            Err(EditError::NotFound("activity"))
        ));
        assert!(matches!(
            set_closed_block_pack(&conn, 10, Some(999)),
            Err(EditError::NotFound("pack"))
        ));
    }

    fn review_db() -> Connection {
        let conn = crate::store::open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO session (id, started_at, ended_at) VALUES (1, 0, 1000), (2, 2000, NULL);",
        )
        .unwrap();
        conn
    }

    fn mmr_rows(conn: &Connection) -> Vec<(u8, i32, Option<i64>)> {
        let mut stmt = conn
            .prepare("SELECT team_size, mmr, session_id FROM mmr_entry ORDER BY id")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn a_review_saves_mmr_notes_and_the_mark_together() {
        let conn = review_db();
        let mmr = [
            MmrInput {
                team_size: 2,
                mmr: 912,
            },
            MmrInput {
                team_size: 1,
                mmr: 632,
            },
        ];
        review_session(&conn, 1, &mmr, Some("  good aerials \u{7} "), 1_500).unwrap();
        assert_eq!(mmr_rows(&conn), [(2, 912, Some(1)), (1, 632, Some(1))]);
        let (notes, reviewed): (String, Option<i64>) = conn
            .query_row(
                "SELECT notes, reviewed_at FROM session WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((notes.as_str(), reviewed), ("good aerials", Some(1_500)));
    }

    #[test]
    fn an_invalid_review_writes_nothing() {
        let conn = review_db();
        let bad = [
            MmrInput {
                team_size: 2,
                mmr: 912,
            },
            MmrInput {
                team_size: 3,
                mmr: 3_001,
            },
        ];
        assert!(matches!(
            review_session(&conn, 1, &bad, None, 1_500),
            Err(EditError::Invalid(_))
        ));
        let twice = [
            MmrInput {
                team_size: 2,
                mmr: 900,
            },
            MmrInput {
                team_size: 2,
                mmr: 910,
            },
        ];
        assert!(matches!(
            review_session(&conn, 1, &twice, None, 1_500),
            Err(EditError::Invalid(_))
        ));
        assert!(mmr_rows(&conn).is_empty());
        let reviewed: Option<i64> = conn
            .query_row("SELECT reviewed_at FROM session WHERE id = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(reviewed, None);
    }

    #[test]
    fn a_session_is_reviewed_once_and_only_when_finished() {
        let conn = review_db();
        review_session(&conn, 1, &[], None, 1_500).unwrap(); // Skip
        let again = review_session(
            &conn,
            1,
            &[MmrInput {
                team_size: 2,
                mmr: 900,
            }],
            None,
            1_600,
        );
        assert!(matches!(again, Err(EditError::Conflict(_))));
        assert!(
            mmr_rows(&conn).is_empty(),
            "the second submit wrote nothing"
        );
        conn.execute(
            "INSERT INTO block (session_id, kind, started_at) VALUES (2, 'match', 2000)",
            [],
        )
        .unwrap(); // an activity of session 2 is still open
        assert!(matches!(
            review_session(&conn, 2, &[], None, 2_500),
            Err(EditError::Conflict(_))
        ));
        assert!(matches!(
            review_session(&conn, 99, &[], None, 2_500),
            Err(EditError::NotFound("session"))
        ));
    }

    #[test]
    fn skipping_an_attempt_of_an_ended_session_says_so() {
        let conn = review_db();
        conn.execute_batch(
            "INSERT INTO block (id, session_id, kind, started_at) VALUES (1, 1, 'training', 0);
             INSERT INTO training_attempt (id, block_id, seq, started_at, duration_ms, outcome)
             VALUES (5, 1, 1, 0, 3000, 'miss');",
        )
        .unwrap();
        let err = set_attempt_skipped(&conn, 5, true, Some(2)).unwrap_err();
        assert_eq!(
            err.to_string(),
            "that attempt is from a session that already ended"
        );
    }

    #[test]
    fn pack_stats_exclude_skips_and_report_last_training() {
        let conn = crate::store::open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO session (id, started_at) VALUES (1, 0);
             INSERT INTO block (id, session_id, kind, started_at, pack_id) VALUES (1, 1, 'training', 0, 6);
             INSERT INTO training_attempt (block_id, seq, started_at, duration_ms, outcome, skipped) VALUES
                (1, 1, 100, 3000, 'make', 0), (1, 2, 200, 3000, 'miss', 0), (1, 3, 300, 3000, 'miss', 1);",
        )
        .unwrap();
        let stats = pack_stats(&conn).unwrap();
        assert_eq!(stats.len(), 11);
        let consistency = stats.iter().find(|s| s.pack.id == 6).unwrap();
        assert_eq!(
            (
                consistency.attempts,
                consistency.makes,
                consistency.make_rate,
                consistency.last_trained_at
            ),
            (2, 1, Some(0.5), Some(200))
        );
        let never = stats.iter().find(|s| s.pack.id == 1).unwrap();
        assert_eq!(
            (never.attempts, never.make_rate, never.last_trained_at),
            (0, None, None)
        );
        assert_eq!(stats[0].pack.name, "Aerial Off Wall", "sorted by name");
    }

    #[test]
    fn an_open_session_is_reviewed_in_the_menus_and_again_after_new_play() {
        let conn = review_db();
        conn.execute(
            "INSERT INTO block (id, session_id, kind, started_at, ended_at) VALUES (31, 2, 'match', 2000, 2500)",
            [],
        )
        .unwrap();
        review_session(
            &conn,
            2,
            &[MmrInput {
                team_size: 2,
                mmr: 900,
            }],
            None,
            2_600,
        )
        .unwrap();
        assert!(matches!(
            review_session(&conn, 2, &[], None, 2_700),
            Err(EditError::Conflict(_))
        ));
        conn.execute(
            "INSERT INTO block (id, session_id, kind, started_at) VALUES (32, 2, 'match', 2800)",
            [],
        )
        .unwrap();
        let busy = review_session(&conn, 2, &[], None, 2_900).unwrap_err();
        assert_eq!(busy.to_string(), "finish the current activity first");
        conn.execute("UPDATE block SET ended_at = 3000 WHERE id = 32", [])
            .unwrap();
        review_session(
            &conn,
            2,
            &[MmrInput {
                team_size: 2,
                mmr: 910,
            }],
            None,
            3_100,
        )
        .unwrap();
        assert_eq!(mmr_rows(&conn), [(2, 900, Some(2)), (2, 910, Some(2))]);
    }

    #[test]
    fn hidden_packs_are_left_out_of_the_phone_list() {
        let conn = db();
        conn.execute("UPDATE pack SET hidden = 1 WHERE id = 6", [])
            .unwrap();
        let stats = pack_stats(&conn).unwrap();
        assert_eq!(stats.len(), 10);
        assert!(stats.iter().all(|s| s.pack.id != 6));
        let row = pack(&conn, 6).unwrap().unwrap();
        assert!(row.hidden);
        assert_eq!(row.notes, "");
        assert_eq!(all_pack_stats(&conn, true).unwrap().len(), 11);
    }

    #[test]
    fn pack_refusals_speak_of_activities() {
        let conn = db();
        let text = |id: i64| {
            set_closed_block_pack(&conn, id, Some(3))
                .unwrap_err()
                .to_string()
        };
        assert_eq!(text(11), "only training activities have a pack");
        assert_eq!(text(20), "the activity is still being recorded");
        assert_eq!(text(99), "activity not found");
    }

    #[test]
    fn moving_an_activity_between_striker_and_goalie_packs_rescores_it() {
        let conn = db();
        conn.execute("UPDATE pack SET goalie = 1 WHERE id = 2", [])
            .unwrap();
        let outcome = || -> String {
            conn.query_row(
                "SELECT outcome FROM training_attempt WHERE id = 100",
                [],
                |r| r.get(0),
            )
            .unwrap()
        };
        set_closed_block_pack(&conn, 10, Some(2)).unwrap();
        assert_eq!(
            outcome(),
            "miss",
            "a goal in a goalie pack is a goal let in"
        );
        set_closed_block_pack(&conn, 10, Some(2)).unwrap();
        assert_eq!(outcome(), "miss", "same pack again: no change");
        set_closed_block_pack(&conn, 10, None).unwrap();
        assert_eq!(outcome(), "make");
    }
}
