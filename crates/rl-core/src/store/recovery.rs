//! Closes sessions left open by a crash, at the latest time the database knows about.

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::store::StoreError;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RecoveryReport {
    pub sessions_closed: usize,
    pub sessions_deleted: usize,
}

/// In-memory freeplay/match aggregates of an interrupted block are lost; the block keeps its
/// time. Empty training blocks and sessions left without blocks are removed, as during recording.
pub fn recover_open_sessions(conn: &mut Connection) -> Result<RecoveryReport, StoreError> {
    let tx = conn.transaction()?;
    let open: Vec<i64> = tx
        .prepare("SELECT id FROM session WHERE ended_at IS NULL")?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let mut report = RecoveryReport::default();
    for id in open {
        let close_at: i64 = tx.query_row(
            "SELECT MAX(s.started_at,
                        COALESCE(s.last_seen_at, 0),
                        COALESCE((SELECT MAX(MAX(b.started_at, COALESCE(b.ended_at, 0)))
                                  FROM block b WHERE b.session_id = s.id), 0),
                        COALESCE((SELECT MAX(a.started_at + a.duration_ms)
                                  FROM training_attempt a JOIN block b ON b.id = a.block_id
                                  WHERE b.session_id = s.id), 0))
             FROM session s WHERE s.id = ?1",
            params![id],
            |r| r.get(0),
        )?;
        tx.execute(
            "UPDATE block SET ended_at = ?1 WHERE session_id = ?2 AND ended_at IS NULL",
            params![close_at, id],
        )?;
        tx.execute(
            "DELETE FROM block WHERE session_id = ?1 AND kind = 'training'
               AND NOT EXISTS (SELECT 1 FROM training_attempt a WHERE a.block_id = block.id)",
            params![id],
        )?;
        let blocks: i64 = tx.query_row(
            "SELECT COUNT(*) FROM block WHERE session_id = ?1",
            params![id],
            |r| r.get(0),
        )?;
        if blocks == 0 {
            tx.execute("DELETE FROM session WHERE id = ?1", params![id])?;
            report.sessions_deleted += 1;
        } else {
            tx.execute(
                "UPDATE session SET ended_at = ?1 WHERE id = ?2",
                params![close_at, id],
            )?;
            report.sessions_closed += 1;
        }
    }
    tx.commit()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::open_in_memory;

    #[test]
    fn closes_open_sessions_at_the_latest_known_time_and_drops_empty_ones() {
        let mut conn = open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO session (id, started_at, ended_at, last_seen_at) VALUES
                 (1, 0, 500, NULL),        -- already closed: untouched
                 (2, 1000, NULL, 50000),   -- heartbeat is the latest time
                 (3, 2000, NULL, NULL),    -- attempt end is the latest time
                 (4, 3000, NULL, 3500);    -- only an empty training block: deleted
             INSERT INTO block (id, session_id, kind, started_at, ended_at) VALUES
                 (20, 2, 'freeplay', 10000, NULL),
                 (30, 3, 'training', 2000, NULL),
                 (40, 4, 'training', 3000, NULL);
             INSERT INTO training_attempt (block_id, seq, started_at, duration_ms, outcome)
                 VALUES (30, 1, 7000, 900, 'make');
             INSERT INTO mmr_entry (team_size, mmr, recorded_at, session_id) VALUES (2, 900, 1, 4);",
        )
        .unwrap();
        let report = recover_open_sessions(&mut conn).unwrap();
        assert_eq!(
            report,
            RecoveryReport {
                sessions_closed: 2,
                sessions_deleted: 1
            }
        );
        let ends: Vec<(i64, Option<i64>)> = conn
            .prepare("SELECT id, ended_at FROM session ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(ends, [(1, Some(500)), (2, Some(50_000)), (3, Some(7_900))]);
        let open_blocks: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM block WHERE ended_at IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(open_blocks, 0);
        let orphan: Option<i64> = conn
            .query_row("SELECT session_id FROM mmr_entry", [], |r| r.get(0))
            .unwrap();
        assert_eq!(orphan, None, "MMR is kept, unlinked");
    }
}
