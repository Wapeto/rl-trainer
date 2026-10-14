//! Desktop edits on finished sessions: review, delete, merge and split. Each runs in one
//! transaction and leaves the database unchanged when it fails.

use rusqlite::{params, Connection, OptionalExtension};

use crate::store::edits::{clean_notes, validate_mmr_list, EditError, MmrInput, TEAM_SIZES};
use crate::store::replays::remember_deleted_matches;
use crate::store::sessions::{neighbour, Neighbour};

struct SessionState {
    started_at: i64,
    ended_at: Option<i64>,
    open_activity: bool,
}

fn session_state(conn: &Connection, id: i64) -> Result<SessionState, EditError> {
    conn.query_row(
        "SELECT s.started_at, s.ended_at,
                EXISTS (SELECT 1 FROM block b WHERE b.session_id = s.id AND b.ended_at IS NULL)
         FROM session s WHERE s.id = ?1",
        params![id],
        |r| {
            Ok(SessionState {
                started_at: r.get(0)?,
                ended_at: r.get(1)?,
                open_activity: r.get(2)?,
            })
        },
    )
    .optional()?
    .ok_or(EditError::NotFound("session"))
}

fn require_finished(state: &SessionState) -> Result<i64, EditError> {
    state
        .ended_at
        .ok_or_else(|| EditError::Conflict("this session is still being recorded".into()))
}

/// Replaces the review: per team size, the session's last entry is updated (or one is added at
/// the session's end); sizes left out lose their entries in this session. Marks it reviewed.
pub fn edit_review(
    conn: &Connection,
    session_id: i64,
    mmr: &[MmrInput],
    notes: &str,
    now_ms: i64,
) -> Result<(), EditError> {
    validate_mmr_list(mmr)?;
    let notes = clean_notes(notes)?;
    let state = session_state(conn, session_id)?;
    if state.open_activity {
        return Err(EditError::Conflict(
            "finish the current activity first".into(),
        ));
    }
    let tx = conn.unchecked_transaction()?;
    for size in TEAM_SIZES {
        match mmr.iter().find(|e| e.team_size == size) {
            Some(entry) => {
                let last: Option<i64> = tx
                    .query_row(
                        "SELECT id FROM mmr_entry WHERE session_id = ?1 AND team_size = ?2
                         ORDER BY recorded_at DESC, id DESC LIMIT 1",
                        params![session_id, size],
                        |r| r.get(0),
                    )
                    .optional()?;
                match last {
                    Some(id) => {
                        tx.execute(
                            "UPDATE mmr_entry SET mmr = ?1 WHERE id = ?2",
                            params![entry.mmr, id],
                        )?;
                    }
                    None => {
                        tx.execute(
                            "INSERT INTO mmr_entry (team_size, mmr, recorded_at, session_id)
                             VALUES (?1, ?2, ?3, ?4)",
                            params![
                                size,
                                entry.mmr,
                                state.ended_at.unwrap_or(now_ms),
                                session_id
                            ],
                        )?;
                    }
                }
            }
            None => {
                tx.execute(
                    "DELETE FROM mmr_entry WHERE session_id = ?1 AND team_size = ?2",
                    params![session_id, size],
                )?;
            }
        }
    }
    tx.execute(
        "UPDATE session SET notes = ?1, reviewed_at = ?2 WHERE id = ?3",
        params![notes, now_ms, session_id],
    )?;
    tx.commit()?;
    Ok(())
}

/// A session can be deleted once it is finished.
pub fn check_delete_session(conn: &Connection, session_id: i64) -> Result<(), EditError> {
    require_finished(&session_state(conn, session_id)?)?;
    Ok(())
}

/// Removes the session with everything recorded in it, MMR entries included.
pub fn apply_delete_session(conn: &Connection, session_id: i64) -> Result<(), EditError> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        &remember_deleted_matches("b.session_id = ?1"),
        params![session_id],
    )?;
    tx.execute(
        "DELETE FROM mmr_entry WHERE session_id = ?1",
        params![session_id],
    )?;
    tx.execute("DELETE FROM session WHERE id = ?1", params![session_id])?;
    tx.commit()?;
    Ok(())
}

pub fn delete_session(conn: &Connection, session_id: i64) -> Result<(), EditError> {
    check_delete_session(conn, session_id)?;
    apply_delete_session(conn, session_id)
}

/// An activity can be deleted once its session is finished. Returns that session's id.
pub fn check_delete_activity(conn: &Connection, block_id: i64) -> Result<i64, EditError> {
    let session_id: i64 = conn
        .query_row(
            "SELECT session_id FROM block WHERE id = ?1",
            params![block_id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or(EditError::NotFound("activity"))?;
    require_finished(&session_state(conn, session_id)?)?;
    Ok(session_id)
}

/// Removes one activity; the session's start and end follow what is left, and the session goes
/// too when nothing is left. Returns whether it went.
pub fn apply_delete_activity(
    conn: &Connection,
    block_id: i64,
    session_id: i64,
) -> Result<bool, EditError> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(&remember_deleted_matches("b.id = ?1"), params![block_id])?;
    tx.execute("DELETE FROM block WHERE id = ?1", params![block_id])?;
    let left: i64 = tx.query_row(
        "SELECT COUNT(*) FROM block WHERE session_id = ?1",
        params![session_id],
        |r| r.get(0),
    )?;
    let session_gone = left == 0;
    if session_gone {
        tx.execute(
            "DELETE FROM mmr_entry WHERE session_id = ?1",
            params![session_id],
        )?;
        tx.execute("DELETE FROM session WHERE id = ?1", params![session_id])?;
    } else {
        tx.execute(
            "UPDATE session SET
                started_at = (SELECT MIN(started_at) FROM block WHERE session_id = ?1),
                ended_at = (SELECT MAX(ended_at) FROM block WHERE session_id = ?1)
             WHERE id = ?1",
            params![session_id],
        )?;
    }
    tx.commit()?;
    Ok(session_gone)
}

pub fn delete_activity(conn: &Connection, block_id: i64) -> Result<bool, EditError> {
    let session_id = check_delete_activity(conn, block_id)?;
    apply_delete_activity(conn, block_id, session_id)
}

fn join_notes(first: &str, second: &str) -> String {
    match (first.is_empty(), second.is_empty()) {
        (false, false) => format!("{first}\n\n{second}"),
        (true, _) => second.to_owned(),
        (false, true) => first.to_owned(),
    }
}

fn notes_of(conn: &Connection, id: i64) -> rusqlite::Result<String> {
    conn.query_row(
        "SELECT notes FROM session WHERE id = ?1",
        params![id],
        |r| r.get(0),
    )
}

/// What a merge will do, decided before anything is written.
pub struct MergePlan {
    keep: i64,
    gone: i64,
    started_at: i64,
    ended_at: i64,
    notes: String,
}

/// Checks a finished session can join its finished neighbour; the earlier one survives.
pub fn plan_merge(
    conn: &Connection,
    session_id: i64,
    with: Neighbour,
) -> Result<MergePlan, EditError> {
    let this = session_state(conn, session_id)?;
    let other_id = neighbour(conn, this.started_at, session_id, with)?.ok_or_else(|| {
        EditError::Conflict(
            match with {
                Neighbour::Previous => "there is no earlier session",
                Neighbour::Next => "there is no later session",
            }
            .into(),
        )
    })?;
    let other = session_state(conn, other_id)?;
    let this_end = require_finished(&this)?;
    let other_end = require_finished(&other).map_err(|_| {
        EditError::Conflict(format!(
            "the {} session is still being recorded",
            match with {
                Neighbour::Previous => "previous",
                Neighbour::Next => "next",
            }
        ))
    })?;
    let (keep, gone) = match with {
        Neighbour::Previous => (other_id, session_id),
        Neighbour::Next => (session_id, other_id),
    };
    Ok(MergePlan {
        keep,
        gone,
        started_at: this.started_at.min(other.started_at),
        ended_at: this_end.max(other_end),
        notes: join_notes(&notes_of(conn, keep)?, &notes_of(conn, gone)?),
    })
}

/// Moves activities and MMR entries into the survivor, joins the notes, marks it reviewed and
/// deletes the other session. Returns the survivor's id.
pub fn apply_merge(conn: &Connection, plan: MergePlan, now_ms: i64) -> Result<i64, EditError> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE block SET session_id = ?1 WHERE session_id = ?2",
        params![plan.keep, plan.gone],
    )?;
    tx.execute(
        "UPDATE mmr_entry SET session_id = ?1 WHERE session_id = ?2",
        params![plan.keep, plan.gone],
    )?;
    tx.execute(
        "UPDATE session SET started_at = ?1, ended_at = ?2, notes = ?3, reviewed_at = ?4
         WHERE id = ?5",
        params![
            plan.started_at,
            plan.ended_at,
            plan.notes,
            now_ms,
            plan.keep
        ],
    )?;
    tx.execute("DELETE FROM session WHERE id = ?1", params![plan.gone])?;
    tx.commit()?;
    Ok(plan.keep)
}

pub fn merge_sessions(
    conn: &Connection,
    session_id: i64,
    with: Neighbour,
    now_ms: i64,
) -> Result<i64, EditError> {
    let plan = plan_merge(conn, session_id, with)?;
    apply_merge(conn, plan, now_ms)
}

/// What a split will do, decided before anything is written.
pub struct SplitPlan {
    session_id: i64,
    /// The activities that move to the new session.
    moved: Vec<i64>,
    /// When the first moved activity started: the new session's start, and the MMR cut.
    gap_at: i64,
    first_end: i64,
    session_end: i64,
}

/// Checks a finished session can be split after `after_block_id` (not its last activity).
pub fn plan_split(
    conn: &Connection,
    session_id: i64,
    after_block_id: i64,
) -> Result<SplitPlan, EditError> {
    let state = session_state(conn, session_id)?;
    let session_end = require_finished(&state)?;
    let mut stmt = conn.prepare(
        "SELECT id, started_at, ended_at FROM block WHERE session_id = ?1 ORDER BY started_at, id",
    )?;
    let blocks: Vec<(i64, i64, Option<i64>)> = stmt
        .query_map(params![session_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?
        .collect::<Result<_, _>>()?;
    let pos = blocks
        .iter()
        .position(|b| b.0 == after_block_id)
        .ok_or(EditError::NotFound("activity"))?;
    if pos + 1 >= blocks.len() {
        return Err(EditError::Invalid(
            "there is nothing after this activity to split off".into(),
        ));
    }
    let (first, second) = blocks.split_at(pos + 1);
    Ok(SplitPlan {
        session_id,
        moved: second.iter().map(|b| b.0).collect(),
        gap_at: second[0].1,
        first_end: first
            .iter()
            .filter_map(|b| b.2)
            .max()
            .unwrap_or(state.started_at),
        session_end,
    })
}

/// Moves the activities after the gap, and the MMR entries recorded from the gap on, to a new
/// session. Both parts count as reviewed; the note stays on the first. Returns the new id.
pub fn apply_split(conn: &Connection, plan: SplitPlan, now_ms: i64) -> Result<i64, EditError> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO session (started_at, ended_at, notes, reviewed_at) VALUES (?1, ?2, '', ?3)",
        params![plan.gap_at, plan.session_end, now_ms],
    )?;
    let new_id = tx.last_insert_rowid();
    for id in &plan.moved {
        tx.execute(
            "UPDATE block SET session_id = ?1 WHERE id = ?2",
            params![new_id, id],
        )?;
    }
    tx.execute(
        "UPDATE mmr_entry SET session_id = ?1 WHERE session_id = ?2 AND recorded_at >= ?3",
        params![new_id, plan.session_id, plan.gap_at],
    )?;
    tx.execute(
        "UPDATE session SET ended_at = ?1, reviewed_at = ?2 WHERE id = ?3",
        params![plan.first_end, now_ms, plan.session_id],
    )?;
    tx.commit()?;
    Ok(new_id)
}

pub fn split_session(
    conn: &Connection,
    session_id: i64,
    after_block_id: i64,
    now_ms: i64,
) -> Result<i64, EditError> {
    let plan = plan_split(conn, session_id, after_block_id)?;
    apply_split(conn, plan, now_ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::open_in_memory;

    /// 1: finished (training 1000–4000, match 5000–9000), 2v2 900 then 905 and a 3v3 700;
    /// 2: open (a finished match, then training still recording).
    fn edits_db() -> Connection {
        let conn = open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO session (id, started_at, ended_at, notes) VALUES
                (1, 1000, 9000, 'old'), (2, 20000, NULL, '');
             INSERT INTO block (id, session_id, kind, started_at, ended_at) VALUES
                (11, 1, 'training', 1000, 4000), (12, 1, 'match', 5000, 9000),
                (21, 2, 'match', 20000, 25000), (22, 2, 'training', 26000, NULL);
             INSERT INTO training_attempt (block_id, seq, started_at, duration_ms, outcome)
                VALUES (11, 1, 1000, 900, 'make');
             INSERT INTO mmr_entry (id, team_size, mmr, recorded_at, session_id) VALUES
                (1, 2, 900, 8000, 1), (2, 2, 905, 9000, 1), (3, 3, 700, 9000, 1);",
        )
        .unwrap();
        conn
    }

    fn mmr_of(conn: &Connection, session: i64) -> Vec<(u8, i32, i64)> {
        let mut stmt = conn
            .prepare(
                "SELECT team_size, mmr, recorded_at FROM mmr_entry WHERE session_id = ?1
                 ORDER BY team_size, recorded_at, id",
            )
            .unwrap();
        stmt.query_map([session], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    fn one<T: rusqlite::types::FromSql>(conn: &Connection, sql: &str) -> T {
        conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn a_review_edit_updates_adds_and_removes_mmr_and_replaces_the_note() {
        let conn = edits_db();
        let mmr = [
            MmrInput {
                team_size: 2,
                mmr: 915,
            },
            MmrInput {
                team_size: 1,
                mmr: 640,
            },
        ];
        edit_review(&conn, 1, &mmr, "  new note ", 50_000).unwrap();
        assert_eq!(
            mmr_of(&conn, 1),
            [(1, 640, 9000), (2, 900, 8000), (2, 915, 9000)],
            "the last 2v2 entry is updated, 1v1 is added at the session end, 3v3 is removed"
        );
        let notes: String = one(&conn, "SELECT notes FROM session WHERE id = 1");
        let reviewed: i64 = one(&conn, "SELECT reviewed_at FROM session WHERE id = 1");
        assert_eq!((notes.as_str(), reviewed), ("new note", 50_000));
    }

    #[test]
    fn a_review_edit_is_refused_while_an_activity_records_and_when_invalid() {
        let conn = edits_db();
        let busy = edit_review(&conn, 2, &[], "x", 1).unwrap_err();
        assert_eq!(busy.to_string(), "finish the current activity first");
        assert!(matches!(
            edit_review(&conn, 99, &[], "", 1),
            Err(EditError::NotFound("session"))
        ));
        let bad = [MmrInput {
            team_size: 4,
            mmr: 900,
        }];
        assert!(matches!(
            edit_review(&conn, 1, &bad, "", 1),
            Err(EditError::Invalid(_))
        ));
        assert_eq!(mmr_of(&conn, 1).len(), 3, "nothing written");
        let notes: String = one(&conn, "SELECT notes FROM session WHERE id = 1");
        assert_eq!(notes, "old");
    }

    #[test]
    fn deleting_a_session_removes_everything_in_it() {
        let conn = edits_db();
        delete_session(&conn, 1).unwrap();
        assert_eq!(
            one::<i64>(&conn, "SELECT COUNT(*) FROM session WHERE id = 1"),
            0
        );
        assert_eq!(
            one::<i64>(&conn, "SELECT COUNT(*) FROM block WHERE session_id = 1"),
            0
        );
        assert_eq!(
            one::<i64>(&conn, "SELECT COUNT(*) FROM training_attempt"),
            0
        );
        assert_eq!(one::<i64>(&conn, "SELECT COUNT(*) FROM mmr_entry"), 0);
        let open = delete_session(&conn, 2).unwrap_err();
        assert_eq!(open.to_string(), "this session is still being recorded");
        assert!(matches!(
            delete_session(&conn, 1),
            Err(EditError::NotFound("session"))
        ));
    }

    #[test]
    fn deleting_activities_shrinks_the_session_then_removes_it() {
        let conn = edits_db();
        assert!(!delete_activity(&conn, 11).unwrap());
        let bounds: (i64, i64) = conn
            .query_row(
                "SELECT started_at, ended_at FROM session WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(bounds, (5000, 9000));
        assert!(
            delete_activity(&conn, 12).unwrap(),
            "the last activity takes the session"
        );
        assert_eq!(
            one::<i64>(&conn, "SELECT COUNT(*) FROM session WHERE id = 1"),
            0
        );
        assert_eq!(one::<i64>(&conn, "SELECT COUNT(*) FROM mmr_entry"), 0);
        assert!(matches!(
            delete_activity(&conn, 21),
            Err(EditError::Conflict(_))
        ));
        assert!(matches!(
            delete_activity(&conn, 999),
            Err(EditError::NotFound("activity"))
        ));
    }

    /// 1: 1000–9000 'a' (11, 12); 2: 20000–30000 'b' (21: 20000–25000, 22: 26000–30000);
    /// 3: open (31). MMR: 2v2 900 in 1 (9000), 905 in 2 (24000), 910 in 2 (30000).
    fn merge_db() -> Connection {
        let conn = open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO session (id, started_at, ended_at, notes) VALUES
                (1, 1000, 9000, 'a'), (2, 20000, 30000, 'b'), (3, 40000, NULL, '');
             INSERT INTO block (id, session_id, kind, started_at, ended_at) VALUES
                (11, 1, 'training', 1000, 4000), (12, 1, 'match', 5000, 9000),
                (21, 2, 'match', 20000, 25000), (22, 2, 'training', 26000, 30000),
                (31, 3, 'match', 40000, NULL);
             INSERT INTO mmr_entry (id, team_size, mmr, recorded_at, session_id) VALUES
                (1, 2, 900, 9000, 1), (3, 2, 905, 24000, 2), (2, 2, 910, 30000, 2);",
        )
        .unwrap();
        conn
    }

    fn blocks_of(conn: &Connection, session: i64) -> Vec<i64> {
        let mut stmt = conn
            .prepare("SELECT id FROM block WHERE session_id = ?1 ORDER BY started_at, id")
            .unwrap();
        stmt.query_map([session], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    fn session_row(conn: &Connection, id: i64) -> (i64, Option<i64>, String, Option<i64>) {
        conn.query_row(
            "SELECT started_at, ended_at, notes, reviewed_at FROM session WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap()
    }

    #[test]
    fn merging_with_the_previous_session_joins_everything_into_the_earlier_one() {
        let conn = merge_db();
        assert_eq!(
            merge_sessions(&conn, 2, Neighbour::Previous, 99).unwrap(),
            1
        );
        assert_eq!(blocks_of(&conn, 1), [11, 12, 21, 22]);
        assert_eq!(mmr_of(&conn, 1).len(), 3);
        assert_eq!(
            session_row(&conn, 1),
            (1000, Some(30000), "a\n\nb".into(), Some(99))
        );
        assert_eq!(
            one::<i64>(&conn, "SELECT COUNT(*) FROM session WHERE id = 2"),
            0
        );
    }

    #[test]
    fn merging_with_the_next_session_keeps_the_earlier_one_too() {
        let conn = merge_db();
        conn.execute("UPDATE session SET notes = '' WHERE id = 2", [])
            .unwrap();
        assert_eq!(merge_sessions(&conn, 1, Neighbour::Next, 99).unwrap(), 1);
        assert_eq!(session_row(&conn, 1).2, "a", "an empty note adds nothing");
    }

    #[test]
    fn merges_that_would_touch_an_open_session_or_nothing_are_refused() {
        let conn = merge_db();
        let first = merge_sessions(&conn, 1, Neighbour::Previous, 99).unwrap_err();
        assert_eq!(first.to_string(), "there is no earlier session");
        let into_open = merge_sessions(&conn, 2, Neighbour::Next, 99).unwrap_err();
        assert_eq!(
            into_open.to_string(),
            "the next session is still being recorded",
            "the refusal names the session that is open"
        );
        let open = merge_sessions(&conn, 3, Neighbour::Previous, 99).unwrap_err();
        assert_eq!(open.to_string(), "this session is still being recorded");
        let last = merge_sessions(&conn, 3, Neighbour::Next, 99).unwrap_err();
        assert_eq!(last.to_string(), "there is no later session");
        assert_eq!(
            one::<i64>(&conn, "SELECT COUNT(*) FROM session"),
            3,
            "nothing changed"
        );
    }

    #[test]
    fn splitting_moves_the_later_activities_and_their_mmr_to_a_new_session() {
        let conn = merge_db();
        let new = split_session(&conn, 2, 21, 77).unwrap();
        assert_eq!(blocks_of(&conn, 2), [21]);
        assert_eq!(blocks_of(&conn, new), [22]);
        assert_eq!(
            session_row(&conn, 2),
            (20000, Some(25000), "b".into(), Some(77))
        );
        assert_eq!(
            session_row(&conn, new),
            (26000, Some(30000), String::new(), Some(77))
        );
        assert_eq!(mmr_of(&conn, 2), [(2, 905, 24000)]);
        assert_eq!(mmr_of(&conn, new), [(2, 910, 30000)]);
    }

    #[test]
    fn splits_that_make_no_sense_are_refused() {
        let conn = merge_db();
        let last = split_session(&conn, 2, 22, 1).unwrap_err();
        assert_eq!(
            last.to_string(),
            "there is nothing after this activity to split off"
        );
        assert!(matches!(
            split_session(&conn, 2, 999, 1),
            Err(EditError::NotFound("activity"))
        ));
        assert!(matches!(
            split_session(&conn, 1, 21, 1),
            Err(EditError::NotFound("activity"))
        ));
        assert!(matches!(
            split_session(&conn, 3, 31, 1),
            Err(EditError::Conflict(_))
        ));
        assert_eq!(one::<i64>(&conn, "SELECT COUNT(*) FROM session"), 3);
    }
}
