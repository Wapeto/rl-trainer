//! Everything the after-session review needs, read in one place (spec §5.1).

use rusqlite::{params, Connection, OptionalExtension};

use crate::coach::review::{build, MatchInput, ModeInput, SessionReview, TrainingInput};
use crate::store::live::block_tries;
use crate::store::progress::stats::{ranked_matches_before, session_ranked_matches, MatchRow};
use crate::store::replays::{stats_for_match, to_save};
use crate::store::sessions::DEFAULT_TARGET_RATE;

/// Earlier ranked matches of a mode used as "usual".
pub const USUAL_MATCHES: i64 = 20;

pub fn session_review(
    conn: &Connection,
    session_id: i64,
    now_ms: i64,
) -> rusqlite::Result<Option<SessionReview>> {
    let ended: Option<Option<i64>> = conn
        .query_row(
            "SELECT ended_at FROM session WHERE id = ?1",
            params![session_id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(ended_at) = ended else {
        return Ok(None);
    };
    let training = training_inputs(conn, session_id)?;
    let modes = mode_inputs(conn, session_id)?;
    let saving = to_save(conn, now_ms)?
        .into_iter()
        .filter(|m| m.session_id == session_id)
        .collect();
    let replay_added_at: Option<i64> = match ended_at {
        Some(end) => conn.query_row(
            "SELECT MAX(r.imported_at) FROM replay_stats r
             JOIN match_record m ON m.id = r.match_id JOIN block b ON b.id = m.block_id
             WHERE b.session_id = ?1 AND r.imported_at > ?2",
            params![session_id, end],
            |r| r.get(0),
        )?,
        None => None,
    };
    Ok(Some(build(
        session_id,
        &training,
        &modes,
        saving,
        replay_added_at,
    )))
}

fn training_inputs(conn: &Connection, session_id: i64) -> rusqlite::Result<Vec<TrainingInput>> {
    let mut stmt = conn.prepare(
        "SELECT b.id, b.pack_id, p.name, COALESCE(p.target_rate, ?2) FROM block b
         LEFT JOIN pack p ON p.id = b.pack_id
         WHERE b.session_id = ?1 AND b.kind = 'training' ORDER BY b.started_at, b.id",
    )?;
    let blocks: Vec<(i64, Option<i64>, Option<String>, f64)> = stmt
        .query_map(params![session_id, DEFAULT_TARGET_RATE], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<Result<_, _>>()?;
    blocks
        .into_iter()
        .map(|(id, pack_id, pack_name, target_rate)| {
            let (tries, goalie) = block_tries(conn, id)?;
            Ok(TrainingInput {
                pack_id,
                pack_name,
                goalie,
                target_rate,
                tries,
            })
        })
        .collect()
}

fn with_replays(conn: &Connection, rows: Vec<MatchRow>) -> rusqlite::Result<Vec<MatchInput>> {
    rows.into_iter()
        .map(|r| {
            let replay = stats_for_match(conn, r.match_id)?;
            Ok((r, replay))
        })
        .collect()
}

fn mode_inputs(conn: &Connection, session_id: i64) -> rusqlite::Result<Vec<ModeInput>> {
    let rows = session_ranked_matches(conn, session_id)?;
    let mut sizes: Vec<i64> = rows
        .iter()
        .map(|r| r.team_size)
        .filter(|s| *s > 0)
        .collect();
    sizes.sort_unstable();
    sizes.dedup();
    sizes
        .into_iter()
        .map(|size| {
            let mine: Vec<MatchRow> = rows
                .iter()
                .filter(|r| r.team_size == size)
                .cloned()
                .collect();
            let first_at = mine.first().map_or(i64::MAX, |r| r.at);
            Ok(ModeInput {
                team_size: size,
                session: with_replays(conn, mine)?,
                earlier: with_replays(
                    conn,
                    ranked_matches_before(conn, size, first_at, USUAL_MATCHES)?,
                )?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::open_in_memory;
    use crate::store::replays::tests::add_match;

    #[test]
    fn a_session_with_training_and_a_ranked_match_is_reviewed() {
        let conn = open_in_memory().unwrap();
        add_match(&conn, 1, 30, true);
        conn.execute_batch(
            "INSERT INTO block (id, session_id, kind, pack_id, started_at) VALUES (50, 1, 'training', 1, 1);
             INSERT INTO training_attempt (block_id, seq, started_at, duration_ms, outcome) VALUES
               (50, 1, 1, 4000, 'make'), (50, 2, 2, 1000, 'miss'), (50, 3, 3, 4000, 'miss');",
        )
        .unwrap();
        let now = 100 * 86_400_000;
        let r = session_review(&conn, 1, now).unwrap().unwrap();
        assert_eq!(r.training.len(), 1);
        assert_eq!((r.training[0].makes, r.training[0].counted), (1, 2));
        assert_eq!(r.modes.len(), 1);
        assert_eq!(r.modes[0].earlier, 0);
        assert_eq!(r.to_save.len(), 1);
        assert_eq!(session_review(&conn, 99, now).unwrap(), None);
    }
}
