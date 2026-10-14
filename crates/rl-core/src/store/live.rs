//! Read queries behind the live snapshot.

use rusqlite::{params, Connection, OptionalExtension};

use crate::coach::rules::Try;
use crate::live::{LiveSession, LiveTraining, MmrLatest, RecentAttempt, SessionSummary};
use crate::store::attempt_sql::{counted, made};
use crate::store::edits::{pack, PackRow};

pub fn session(conn: &Connection, id: i64) -> rusqlite::Result<Option<LiveSession>> {
    conn.query_row(
        "SELECT id, started_at, notes FROM session WHERE id = ?1",
        params![id],
        |r| {
            Ok(LiveSession {
                id: r.get(0)?,
                started_at: r.get(1)?,
                notes: r.get(2)?,
            })
        },
    )
    .optional()
}

/// The pack of the most recent other training block of the session that has one; `None` when
/// that pack is hidden (an older pack is not offered in its place).
pub fn suggested_pack(
    conn: &Connection,
    session_id: i64,
    open_block_id: i64,
) -> rusqlite::Result<Option<PackRow>> {
    let latest: Option<(i64, bool)> = conn
        .query_row(
            "SELECT b.pack_id, p.hidden FROM block b JOIN pack p ON p.id = b.pack_id
             WHERE b.session_id = ?1 AND b.id != ?2 AND b.kind = 'training'
             ORDER BY b.started_at DESC, b.id DESC LIMIT 1",
            params![session_id, open_block_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    match latest {
        Some((id, false)) => pack(conn, id),
        _ => Ok(None),
    }
}

pub fn training(conn: &Connection, block_id: i64) -> rusqlite::Result<LiveTraining> {
    let (attempts, makes): (i64, i64) = conn.query_row(
        &format!(
            "SELECT COUNT(*), COALESCE(SUM({}), 0) FROM training_attempt a
             WHERE a.block_id = ?1 AND {}",
            made("a"),
            counted("a")
        ),
        params![block_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let mut stmt = conn.prepare(
        "SELECT id, seq, outcome, skipped FROM training_attempt
         WHERE block_id = ?1 ORDER BY seq DESC LIMIT 10",
    )?;
    let mut recent: Vec<RecentAttempt> = stmt
        .query_map(params![block_id], |r| {
            Ok(RecentAttempt {
                id: r.get(0)?,
                seq: r.get(1)?,
                outcome: r.get(2)?,
                skipped: r.get(3)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    recent.reverse();
    let mut newest_first = conn.prepare(&format!(
        "SELECT {} FROM training_attempt a
         WHERE a.block_id = ?1 AND {} ORDER BY a.seq DESC",
        made("a"),
        counted("a")
    ))?;
    let mut rows = newest_first.query(params![block_id])?;
    let mut streak = 0;
    while let Some(row) = rows.next()? {
        if !row.get::<_, bool>(0)? {
            break;
        }
        streak += 1;
    }
    let (tries, goalie) = block_tries(conn, block_id)?;
    let nudge = crate::coach::nudges::evaluate(&tries, goalie).active;
    Ok(LiveTraining {
        attempts,
        makes,
        make_rate: (attempts > 0).then(|| makes as f64 / attempts as f64),
        streak,
        recent,
        nudge,
    })
}

/// The block's attempts in order, and whether its pack is a goalie pack.
pub(crate) fn block_tries(conn: &Connection, block_id: i64) -> rusqlite::Result<(Vec<Try>, bool)> {
    let goalie: bool = conn
        .query_row(
            "SELECT COALESCE(p.goalie, 0) FROM block b LEFT JOIN pack p ON p.id = b.pack_id
             WHERE b.id = ?1",
            params![block_id],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(false);
    let mut stmt = conn.prepare(&format!(
        "SELECT id, started_at, {}, duration_ms, skipped FROM training_attempt
         WHERE block_id = ?1 ORDER BY seq",
        made("training_attempt")
    ))?;
    let tries = stmt
        .query_map(params![block_id], |r| {
            Ok(Try {
                id: r.get(0)?,
                started_at: r.get(1)?,
                made: r.get(2)?,
                duration_ms: r.get(3)?,
                skipped: r.get(4)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok((tries, goalie))
}

/// Wins and losses today in one playlist.
pub fn today_record(
    conn: &Connection,
    playlist_id: Option<i32>,
    day_start_ms: i64,
) -> rusqlite::Result<(i64, i64)> {
    conn.query_row(
        "SELECT COALESCE(SUM(m.result = 'win'), 0), COALESCE(SUM(m.result = 'loss'), 0)
         FROM match_record m JOIN block b ON b.id = m.block_id
         WHERE m.playlist_id IS ?1 AND b.started_at >= ?2",
        params![playlist_id, day_start_ms],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
}

pub fn latest_mmr(conn: &Connection) -> rusqlite::Result<Vec<MmrLatest>> {
    let mut stmt = conn.prepare(
        "SELECT m.team_size, m.mmr, m.recorded_at FROM mmr_entry m
         WHERE m.id = (SELECT id FROM mmr_entry WHERE team_size = m.team_size
                       ORDER BY recorded_at DESC, id DESC LIMIT 1)
         ORDER BY m.team_size",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(MmrLatest {
            team_size: r.get(0)?,
            mmr: r.get(1)?,
            recorded_at: r.get(2)?,
        })
    })?;
    rows.collect()
}

pub fn last_session(conn: &Connection) -> rusqlite::Result<Option<SessionSummary>> {
    let id: Option<i64> = conn
        .query_row(
            "SELECT id FROM session WHERE ended_at IS NOT NULL ORDER BY ended_at DESC, id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?;
    id.map_or(Ok(None), |id| session_summary(conn, id))
}

/// Summary of a finished session; `None` while it is open or if it does not exist.
pub fn session_summary(conn: &Connection, id: i64) -> rusqlite::Result<Option<SessionSummary>> {
    summary(conn, id, true)
}

/// Summary of the open session so far; its `ended_at` is the end of its latest finished
/// activity (when the menus began, if none is open). `None` once it has finished.
pub fn current_session(conn: &Connection, id: i64) -> rusqlite::Result<Option<SessionSummary>> {
    summary(conn, id, false)
}

/// A review covers what ended before `reviewed_at`; anything that ended later needs another.
fn summary(conn: &Connection, id: i64, finished: bool) -> rusqlite::Result<Option<SessionSummary>> {
    let head: Option<(i64, i64, String, Option<i64>)> = conn
        .query_row(
            "SELECT s.started_at,
                    COALESCE(s.ended_at,
                             (SELECT MAX(b.ended_at) FROM block b WHERE b.session_id = s.id),
                             s.started_at),
                    s.notes, s.reviewed_at
             FROM session s WHERE s.id = ?1 AND (s.ended_at IS NOT NULL) = ?2",
            params![id, finished],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((started_at, ended_at, notes, reviewed_at)) = head else {
        return Ok(None);
    };
    let reviewed = match reviewed_at {
        None => false,
        Some(at) => !conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM block WHERE session_id = ?1 AND ended_at > ?2)",
            params![id, at],
            |r| r.get::<_, bool>(0),
        )?,
    };
    let count = |kind: &str| -> rusqlite::Result<i64> {
        conn.query_row(
            "SELECT COUNT(*) FROM block WHERE session_id = ?1 AND kind = ?2",
            params![id, kind],
            |r| r.get(0),
        )
    };
    let (wins, losses): (i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(m.result = 'win'), 0), COALESCE(SUM(m.result = 'loss'), 0)
         FROM match_record m JOIN block b ON b.id = m.block_id WHERE b.session_id = ?1",
        params![id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let mut stmt = conn.prepare(
        "SELECT DISTINCT m.team_size FROM match_record m JOIN block b ON b.id = m.block_id
         WHERE b.session_id = ?1 AND m.ranked = 1 AND m.team_size IS NOT NULL
           AND b.ended_at > ?2
         ORDER BY m.team_size",
    )?;
    let ranked_team_sizes = stmt
        .query_map(params![id, reviewed_at.unwrap_or(i64::MIN)], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let (attempts, makes): (i64, i64) = conn.query_row(
        &format!(
            "SELECT COUNT(*), COALESCE(SUM({}), 0)
             FROM training_attempt a JOIN block b ON b.id = a.block_id
             WHERE b.session_id = ?1 AND {}",
            made("a"),
            counted("a")
        ),
        params![id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let training_target_rate: Option<f64> = conn.query_row(
        &format!(
            "SELECT SUM(COALESCE(p.target_rate, ?2) * x.n) / SUM(x.n)
             FROM (SELECT b.pack_id, COUNT(*) AS n
                   FROM training_attempt a JOIN block b ON b.id = a.block_id
                   WHERE b.session_id = ?1 AND b.kind = 'training' AND {}
                   GROUP BY b.id) x
             LEFT JOIN pack p ON p.id = x.pack_id",
            counted("a")
        ),
        params![id, crate::store::sessions::DEFAULT_TARGET_RATE],
        |r| r.get(0),
    )?;
    let training_blocks = count("training")?;
    let freeplay_blocks = count("freeplay")?;
    let match_blocks = count("match")?;
    let other_blocks = count("other")?;
    Ok(Some(SessionSummary {
        id,
        started_at,
        ended_at,
        training_blocks,
        freeplay_blocks,
        match_blocks,
        other_blocks,
        wins,
        losses,
        ranked_team_sizes,
        notes,
        activities: training_blocks + freeplay_blocks + match_blocks + other_blocks,
        training_make_rate: (attempts > 0).then(|| makes as f64 / attempts as f64),
        training_target_rate,
        reviewed,
    }))
}

/// Hardest free-play hit of the stretches that started at or after `since_ms` (finished ones only;
/// the open stretch is added by the caller).
pub fn best_freeplay_hit(conn: &Connection, since_ms: i64) -> rusqlite::Result<Option<f64>> {
    conn.query_row(
        "SELECT MAX(f.max_hit_speed) FROM freeplay_stats f JOIN block b ON b.id = f.block_id
         WHERE b.started_at >= ?1",
        params![since_ms],
        |r| r.get(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::open_in_memory;

    #[test]
    fn best_free_play_hit_counts_only_stretches_since_the_given_time() {
        let conn = open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO session (id, started_at) VALUES (1, 0);
             INSERT INTO block (id, session_id, kind, started_at) VALUES
                (1, 1, 'freeplay', 100), (2, 1, 'freeplay', 5000);
             INSERT INTO freeplay_stats (block_id, touches, hard_hits, avg_hit_speed, max_hit_speed,
                goals, crossbar_hits, pct_supersonic, pct_on_wall, pct_airborne)
             VALUES (1, 1, 1, 120, 120, 0, 0, 0, 0, 0), (2, 1, 1, 95, 95, 0, 0, 0, 0, 0);",
        )
        .unwrap();
        assert_eq!(best_freeplay_hit(&conn, 0).unwrap(), Some(120.0));
        assert_eq!(best_freeplay_hit(&conn, 1_000).unwrap(), Some(95.0));
        assert_eq!(best_freeplay_hit(&conn, 9_000).unwrap(), None);
    }

    fn session_db() -> Connection {
        let conn = open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO session (id, started_at, ended_at) VALUES (1, 0, 9000);
             UPDATE pack SET target_rate = 0.5 WHERE id = 2;
             INSERT INTO block (id, session_id, kind, started_at, pack_id) VALUES
                (1, 1, 'training', 0, 1), (2, 1, 'training', 100, 2), (3, 1, 'freeplay', 200, NULL);
             INSERT INTO training_attempt (block_id, seq, started_at, duration_ms, outcome, skipped) VALUES
                (1, 1, 0, 3000, 'miss', 0), (1, 2, 0, 3000, 'make', 0), (1, 3, 0, 3000, 'make', 0),
                (1, 4, 0, 3000, 'miss', 1), (2, 1, 0, 3000, 'make', 0);",
        )
        .unwrap();
        conn
    }

    #[test]
    fn streak_counts_makes_back_from_the_newest_ignoring_skips() {
        let conn = session_db();
        assert_eq!(training(&conn, 1).unwrap().streak, 2);
        conn.execute(
            "UPDATE training_attempt SET skipped = 0 WHERE block_id = 1 AND seq = 4",
            [],
        )
        .unwrap();
        assert_eq!(training(&conn, 1).unwrap().streak, 0);
    }

    #[test]
    fn session_summary_has_activities_training_rate_target_and_review_state() {
        let conn = session_db();
        let s = session_summary(&conn, 1).unwrap().unwrap();
        assert_eq!(s.activities, 3);
        assert_eq!(s.training_make_rate, Some(0.75));
        // Pack 1 (0.65) has 3 counted attempts, pack 2 (0.5) has 1.
        assert!((s.training_target_rate.unwrap() - (0.65 * 3.0 + 0.5) / 4.0).abs() < 1e-9);
        assert!(!s.reviewed);
        conn.execute("UPDATE session SET reviewed_at = 9500 WHERE id = 1", [])
            .unwrap();
        assert!(last_session(&conn).unwrap().unwrap().reviewed);
    }

    #[test]
    fn open_or_unknown_sessions_have_no_summary() {
        let conn = session_db();
        conn.execute("INSERT INTO session (id, started_at) VALUES (2, 10000)", [])
            .unwrap();
        assert!(session_summary(&conn, 2).unwrap().is_none());
        assert!(session_summary(&conn, 99).unwrap().is_none());
    }

    /// An open session with a 2v2 match (ended 1000) and a 3v3 match (ended 3000).
    fn open_session_with_matches(conn: &Connection) {
        conn.execute_batch(
            "INSERT INTO session (id, started_at) VALUES (5, 0);
             INSERT INTO block (id, session_id, kind, started_at, ended_at) VALUES
                (21, 5, 'match', 0, 1000), (22, 5, 'match', 1500, 3000);
             INSERT INTO match_record (block_id, match_guid, ranked, team_size, arena, result,
                team_score, opponent_score, overtime, score, goals, assists, saves, shots, touches,
                demos, epic_saves, avg_boost, pct_zero_boost, pct_supersonic, metrics_partial,
                scoreboard_json)
             VALUES (21, 'a', 1, 2, 'x', 'win', 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, '[]'),
                    (22, 'b', 1, 3, 'x', 'loss', 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, '[]');",
        )
        .unwrap();
    }

    #[test]
    fn the_open_session_has_a_summary_up_to_its_last_finished_activity() {
        let conn = open_in_memory().unwrap();
        open_session_with_matches(&conn);
        conn.execute(
            "INSERT INTO block (session_id, kind, started_at) VALUES (5, 'freeplay', 3500)",
            [],
        )
        .unwrap(); // still open
        let s = current_session(&conn, 5).unwrap().unwrap();
        assert_eq!(
            (s.ended_at, s.activities, s.wins, s.losses),
            (3000, 3, 1, 1)
        );
        assert_eq!(s.ranked_team_sizes, [2, 3]);
        assert!(!s.reviewed);
        assert!(session_summary(&conn, 5).unwrap().is_none(), "not finished");
    }

    #[test]
    fn a_review_covers_what_ended_before_it_and_only_new_ranked_play_is_asked() {
        let conn = open_in_memory().unwrap();
        open_session_with_matches(&conn);
        conn.execute("UPDATE session SET reviewed_at = 2000 WHERE id = 5", [])
            .unwrap();
        let s = current_session(&conn, 5).unwrap().unwrap();
        assert!(!s.reviewed, "the 3v3 match ended after the review");
        assert_eq!(s.ranked_team_sizes, [3]);
        conn.execute("UPDATE session SET reviewed_at = 3500 WHERE id = 5", [])
            .unwrap();
        let s = current_session(&conn, 5).unwrap().unwrap();
        assert!(s.reviewed);
        assert!(s.ranked_team_sizes.is_empty());
        conn.execute("UPDATE session SET ended_at = 4000 WHERE id = 5", [])
            .unwrap();
        assert!(
            session_summary(&conn, 5).unwrap().unwrap().reviewed,
            "quitting without new play asks nothing more"
        );
    }

    #[test]
    fn a_hidden_latest_pack_is_not_replaced_by_an_older_one() {
        let conn = open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO session (id, started_at) VALUES (1, 0);
             INSERT INTO block (id, session_id, kind, started_at, ended_at, pack_id) VALUES
                (1, 1, 'training', 0, 50, 2), (2, 1, 'training', 60, 90, 3),
                (3, 1, 'training', 100, NULL, NULL);
             UPDATE pack SET hidden = 1 WHERE id = 3;",
        )
        .unwrap();
        assert!(
            suggested_pack(&conn, 1, 3).unwrap().is_none(),
            "the last pack played is hidden: no suggestion, not the one before it"
        );
    }

    #[test]
    fn a_hidden_pack_is_never_suggested() {
        let conn = open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO session (id, started_at) VALUES (1, 0);
             INSERT INTO block (id, session_id, kind, started_at, ended_at, pack_id) VALUES
                (1, 1, 'training', 0, 50, 3), (2, 1, 'training', 60, NULL, NULL);
             UPDATE pack SET hidden = 1 WHERE id = 3;",
        )
        .unwrap();
        assert!(suggested_pack(&conn, 1, 2).unwrap().is_none());
        conn.execute("UPDATE pack SET hidden = 0 WHERE id = 3", [])
            .unwrap();
        assert_eq!(suggested_pack(&conn, 1, 2).unwrap().map(|p| p.id), Some(3));
    }

    #[test]
    fn unassigned_training_counts_toward_the_target_at_the_default_rate() {
        let conn = session_db();
        conn.execute_batch(
            "INSERT INTO block (id, session_id, kind, started_at) VALUES (4, 1, 'training', 300);
             INSERT INTO training_attempt (block_id, seq, started_at, duration_ms, outcome) VALUES
                (4, 1, 0, 3000, 'miss'), (4, 2, 0, 3000, 'miss'), (4, 3, 0, 3000, 'miss'),
                (4, 4, 0, 3000, 'miss');",
        )
        .unwrap();
        let s = session_summary(&conn, 1).unwrap().unwrap();
        let expected = (0.65 * 3.0 + 0.5 + 0.65 * 4.0) / 8.0;
        assert!((s.training_target_rate.unwrap() - expected).abs() < 1e-9);
    }

    #[test]
    fn the_live_training_carries_the_active_nudge() {
        let conn = open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO session (id, started_at, last_seen_at) VALUES (1, 0, 0);
             INSERT INTO block (id, session_id, kind, pack_id, started_at) VALUES (1, 1, 'training', 1, 0);",
        )
        .unwrap();
        for seq in 1..=12 {
            let outcome = if seq <= 2 { "make" } else { "miss" };
            conn.execute(
                "INSERT INTO training_attempt (block_id, seq, started_at, duration_ms, outcome)
                 VALUES (1, ?1, ?1, 4000, ?2)",
                params![seq, outcome],
            )
            .unwrap();
        }
        let live = training(&conn, 1).unwrap();
        assert!(matches!(
            live.nudge,
            Some(crate::coach::nudges::Nudge::MissStreak { count: 10, .. })
        ));
    }
}
