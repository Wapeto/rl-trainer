//! Goalie packs. The tracker scores every shot as a striker shot: the ball going in is a make,
//! anything else (the timer, a reset) a miss. In a goalie pack the ball going in is a goal let
//! in and anything else a save, so its shots are stored with the other outcome. Moving shots
//! between a striker and a goalie pack swaps their outcomes, which is exact both ways.

use rusqlite::{params, Connection, OptionalExtension};

use crate::facts::AttemptOutcome;

/// Whether shots in this pack are scored as a goalie's.
pub(crate) fn is_goalie(conn: &Connection, pack_id: Option<i64>) -> rusqlite::Result<bool> {
    let Some(id) = pack_id else {
        return Ok(false);
    };
    Ok(conn
        .query_row("SELECT goalie FROM pack WHERE id = ?1", params![id], |r| {
            r.get(0)
        })
        .optional()?
        .unwrap_or(false))
}

/// A shot the tracker scored as a striker shot, scored for the pack it was played in.
pub(crate) fn scored(striker: AttemptOutcome, goalie: bool) -> AttemptOutcome {
    match (striker, goalie) {
        (outcome, false) => outcome,
        (AttemptOutcome::Make, true) => AttemptOutcome::Miss,
        (AttemptOutcome::Miss, true) => AttemptOutcome::Make,
    }
}

const SWAP: &str = "outcome = CASE outcome WHEN 'make' THEN 'miss' ELSE 'make' END";

/// Rescores one activity when it moves between a striker and a goalie pack.
pub(crate) fn rescore_block(
    conn: &Connection,
    block_id: i64,
    was_goalie: bool,
    goalie: bool,
) -> rusqlite::Result<()> {
    if was_goalie != goalie {
        conn.execute(
            &format!("UPDATE training_attempt SET {SWAP} WHERE block_id = ?1"),
            params![block_id],
        )?;
    }
    Ok(())
}

/// Rescores every activity of a pack whose kind changed.
pub(crate) fn rescore_pack(
    conn: &Connection,
    pack_id: i64,
    was_goalie: bool,
    goalie: bool,
) -> rusqlite::Result<()> {
    if was_goalie != goalie {
        conn.execute(
            &format!(
                "UPDATE training_attempt SET {SWAP}
                 WHERE block_id IN (SELECT id FROM block WHERE pack_id = ?1)"
            ),
            params![pack_id],
        )?;
    }
    Ok(())
}
