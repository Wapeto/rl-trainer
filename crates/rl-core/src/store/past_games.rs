//! What a replay's header adds to the log: the final result of a recorded match, and games that
//! were never recorded live ("Import past games").

use std::collections::HashSet;

use rusqlite::{params, Connection, OptionalExtension};

/// The final score of a match, seen from my team.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplayOutcome {
    pub my_score: i64,
    pub their_score: i64,
}

impl ReplayOutcome {
    fn result(self) -> &'static str {
        match self.my_score.cmp(&self.their_score) {
            std::cmp::Ordering::Greater => "win",
            std::cmp::Ordering::Less => "loss",
            std::cmp::Ordering::Equal => "unknown",
        }
    }
}

/// Lets the replay's score replace the live one. Only a `live` result is replaced (never a
/// `manual` one), and a tied score says nothing about who won, so it changes nothing. Returns
/// whether the row changed; running it again changes nothing.
pub fn apply_outcome(
    conn: &Connection,
    match_id: i64,
    outcome: ReplayOutcome,
) -> rusqlite::Result<bool> {
    let changed = conn.execute(
        "UPDATE match_record
         SET result = ?2, team_score = ?3, opponent_score = ?4, result_source = 'replay'
         WHERE id = ?1 AND result_source = 'live' AND ?3 <> ?4",
        params![
            match_id,
            outcome.result(),
            outcome.my_score,
            outcome.their_score
        ],
    )?;
    Ok(changed > 0)
}

/// Replay files that were looked at for good: imported, not mine, or unreadable.
pub fn settled_files(conn: &Connection) -> rusqlite::Result<HashSet<String>> {
    let mut stmt = conn.prepare(
        "SELECT path FROM replay_file WHERE status IN ('imported', 'not_mine', 'error')",
    )?;
    let rows = stmt.query_map([], |r| r.get(0))?;
    rows.collect()
}

fn is_deleted(conn: &Connection, guid: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM deleted_match WHERE match_guid = ?1)",
        params![guid],
        |r| r.get(0),
    )
}

/// A match deleted before 0.3 kept only its start time. Its live block starts close to the
/// replay's estimated start (save time minus the game length), so match on a window.
const DELETED_START_SLACK_MS: i64 = 120_000;

fn deleted_without_guid(conn: &Connection, game: &PastGame) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM deleted_match
                        WHERE match_guid IS NULL AND started_at BETWEEN ?1 AND ?2)",
        params![game.started_at - DELETED_START_SLACK_MS, game.ended_at],
        |r| r.get(0),
    )
}

/// Whether the replay of this game needs no more reading: the user deleted the match, or its
/// replay stats are already saved. (A live match without stats is the folder poller's job.)
pub fn game_settled(conn: &Connection, guid: &str) -> rusqlite::Result<bool> {
    if is_deleted(conn, guid)? {
        return Ok(true);
    }
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM match_record m JOIN replay_stats r ON r.match_id = m.id
                        WHERE m.match_guid = ?1)",
        params![guid],
        |r| r.get(0),
    )
}

/// One game read from a replay, to be added to the log.
#[derive(Debug, Clone, PartialEq)]
pub struct PastGame {
    pub match_guid: String,
    pub started_at: i64,
    pub ended_at: i64,
    pub team_size: Option<i64>,
    pub arena: String,
    pub outcome: ReplayOutcome,
    pub score: i64,
    pub goals: i64,
    pub assists: i64,
    pub saves: i64,
    pub shots: i64,
    pub scoreboard_json: String,
}

/// Adds the game as a match block in the replay session of its local day (`day_start_ms` up to
/// `day_end_ms`), making that session when the day has none. Returns the new match id, or `None`
/// when a match with this GUID is already in the log or was deleted from it. Call it inside a
/// transaction.
pub fn insert_past_game(
    conn: &Connection,
    game: &PastGame,
    day_start_ms: i64,
    day_end_ms: i64,
) -> rusqlite::Result<Option<i64>> {
    if super::replays::match_id_by_guid(conn, &game.match_guid)?.is_some() {
        return Ok(None);
    }
    if is_deleted(conn, &game.match_guid)? || deleted_without_guid(conn, game)? {
        return Ok(None);
    }
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM session
             WHERE source = 'replay' AND started_at >= ?1 AND started_at < ?2 LIMIT 1",
            params![day_start_ms, day_end_ms],
            |r| r.get(0),
        )
        .optional()?;
    let session_id = match existing {
        Some(id) => {
            conn.execute(
                "UPDATE session SET started_at = MIN(started_at, ?2),
                     ended_at = MAX(COALESCE(ended_at, 0), ?3) WHERE id = ?1",
                params![id, game.started_at, game.ended_at],
            )?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO session (started_at, ended_at, source) VALUES (?1, ?2, 'replay')",
                params![game.started_at, game.ended_at],
            )?;
            conn.last_insert_rowid()
        }
    };
    conn.execute(
        "INSERT INTO block (session_id, kind, started_at, ended_at)
         VALUES (?1, 'match', ?2, ?3)",
        params![session_id, game.started_at, game.ended_at],
    )?;
    let block_id = conn.last_insert_rowid();
    // Not recorded live: no touches, boost or other live-feed numbers (`metrics_partial`), and
    // the replay says nothing about ranked play (casual is the fallback).
    conn.execute(
        "INSERT INTO match_record (block_id, match_guid, ranked, team_size, arena, result,
             team_score, opponent_score, overtime, score, goals, assists, saves, shots, touches,
             demos, epic_saves, avg_boost, pct_zero_boost, pct_supersonic, metrics_partial,
             scoreboard_json, result_source)
         VALUES (?1, ?2, 0, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9, ?10, ?11, ?12, 0, 0, 0, 0, 0, 0, 1,
             ?13, 'replay')",
        params![
            block_id,
            game.match_guid,
            game.team_size,
            game.arena,
            game.outcome.result(),
            game.outcome.my_score,
            game.outcome.their_score,
            game.score,
            game.goals,
            game.assists,
            game.saves,
            game.shots,
            game.scoreboard_json
        ],
    )?;
    Ok(Some(conn.last_insert_rowid()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::open_in_memory;
    use crate::store::replays::tests::add_match;
    use crate::store::replays::{recent_pool, save_players, PlayerReplayStats, ReplayPlayer};

    const DAY: i64 = 86_400_000;

    fn game(guid: &str, started_at: i64) -> PastGame {
        PastGame {
            match_guid: guid.into(),
            started_at,
            ended_at: started_at + 300_000,
            team_size: Some(2),
            arena: "Park_P".into(),
            outcome: ReplayOutcome {
                my_score: 3,
                their_score: 1,
            },
            score: 400,
            goals: 2,
            assists: 0,
            saves: 1,
            shots: 3,
            scoreboard_json: "[]".into(),
        }
    }

    fn result_of(conn: &Connection, id: i64) -> (String, String) {
        conn.query_row(
            "SELECT result, result_source FROM match_record WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
    }

    const LOSS: ReplayOutcome = ReplayOutcome {
        my_score: 1,
        their_score: 2,
    };

    #[test]
    fn a_replay_result_replaces_the_live_one_once() {
        let conn = open_in_memory().unwrap();
        add_match(&conn, 1, 60, true); // a live win, 3-1
        assert!(apply_outcome(&conn, 1, LOSS).unwrap());
        assert_eq!(result_of(&conn, 1), ("loss".into(), "replay".into()));
        let scores: (i64, i64) = conn
            .query_row(
                "SELECT team_score, opponent_score FROM match_record WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(scores, (1, 2));
        assert!(!apply_outcome(&conn, 1, LOSS).unwrap(), "importing again");
    }

    #[test]
    fn a_manual_result_and_a_tied_score_are_left_alone() {
        let conn = open_in_memory().unwrap();
        add_match(&conn, 1, 60, true);
        add_match(&conn, 2, 50, true);
        conn.execute(
            "UPDATE match_record SET result_source = 'manual' WHERE id = 1",
            [],
        )
        .unwrap();
        assert!(!apply_outcome(&conn, 1, LOSS).unwrap());
        assert_eq!(result_of(&conn, 1), ("win".into(), "manual".into()));
        let tie = ReplayOutcome {
            my_score: 2,
            their_score: 2,
        };
        assert!(!apply_outcome(&conn, 2, tie).unwrap());
        assert_eq!(result_of(&conn, 2), ("win".into(), "live".into()));
    }

    #[test]
    fn the_progress_pool_sees_the_replay_result() {
        let conn = open_in_memory().unwrap();
        add_match(&conn, 1, 60, true);
        let me = |team| ReplayPlayer {
            slot: team,
            team,
            name: "Me".into(),
            is_me: team == 0,
            is_bot: false,
            stats: PlayerReplayStats::default(),
        };
        save_players(&conn, 1, &[me(0), me(1)]).unwrap();
        let won = |c: &Connection| {
            recent_pool(c, 2)
                .unwrap()
                .1
                .iter()
                .find(|r| r.is_me)
                .unwrap()
                .won
        };
        assert!(won(&conn));
        apply_outcome(&conn, 1, LOSS).unwrap();
        assert!(!won(&conn));
    }

    #[test]
    fn two_games_on_one_day_share_a_session() {
        let conn = open_in_memory().unwrap();
        let day = 50 * DAY;
        let (a, b) = (
            game("A", day + 10 * 3_600_000),
            game("B", day + 12 * 3_600_000),
        );
        let first = insert_past_game(&conn, &a, day, day + DAY).unwrap();
        let second = insert_past_game(&conn, &b, day, day + DAY).unwrap();
        assert!(first.is_some() && second.is_some());
        let count = |sql: &str| -> i64 { conn.query_row(sql, [], |r| r.get(0)).unwrap() };
        assert_eq!(
            count("SELECT COUNT(*) FROM session WHERE source = 'replay'"),
            1
        );
        assert_eq!(count("SELECT COUNT(*) FROM block WHERE kind = 'match'"), 2);
        let (start, end): (i64, i64) = conn
            .query_row("SELECT started_at, ended_at FROM session", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!((start, end), (a.started_at, b.ended_at));
        let next = insert_past_game(&conn, &game("C", day + DAY + 1), day + DAY, day + 2 * DAY);
        assert!(next.unwrap().is_some());
        assert_eq!(count("SELECT COUNT(*) FROM session"), 2);
    }

    #[test]
    fn a_guid_already_in_the_log_is_skipped() {
        let conn = open_in_memory().unwrap();
        add_match(&conn, 1, 60, true); // guid "GUID1"
        let skipped = insert_past_game(&conn, &game("GUID1", 5), 0, DAY).unwrap();
        assert_eq!(skipped, None);
        let matches: i64 = conn
            .query_row("SELECT COUNT(*) FROM match_record", [], |r| r.get(0))
            .unwrap();
        assert_eq!(matches, 1);
    }

    #[test]
    fn a_match_deleted_before_guids_were_kept_stays_gone() {
        let conn = open_in_memory().unwrap();
        let start = 10 * DAY;
        // Deleted under 0.2: only the live block's start is known, a little after the estimate.
        conn.execute(
            "INSERT INTO deleted_match (started_at, match_guid) VALUES (?1, NULL)",
            params![start + 30_000],
        )
        .unwrap();
        let gone = insert_past_game(&conn, &game("OLD", start), 9 * DAY, 11 * DAY).unwrap();
        assert_eq!(gone, None);
        let other = insert_past_game(&conn, &game("NEW", start + 3_600_000), 9 * DAY, 11 * DAY);
        assert!(
            other.unwrap().is_some(),
            "a game an hour later is not the deleted one"
        );
    }

    #[test]
    fn an_imported_session_never_needs_review() {
        let conn = open_in_memory().unwrap();
        insert_past_game(&conn, &game("A", 1_000), 0, DAY).unwrap();
        let cards = crate::store::sessions::session_cards(&conn, "", 0, 10).unwrap();
        assert!(cards[0].imported && !cards[0].needs_review);
        assert_eq!(cards[0].casual_matches, 1);
    }
}
