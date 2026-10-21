//! SQLite persistence.

mod attempt_sql;
pub mod edits;
pub mod live;
pub mod migrations;
pub mod packs;
pub mod past_games;
pub mod progress;
pub mod queries;
mod recorder;
pub mod recovery;
pub mod replays;
pub mod review;
mod scoring;
pub mod session_edits;
pub mod sessions;

pub use edits::{EditError, MmrInput, PackRow, PackStats};
pub use queries::{
    clear_active_profile, list_profiles, list_sessions, profile_primary_id, set_active_profile,
    BlockRow, ProfileRow, SessionRow,
};
pub use recorder::{Cursor, Recorder};
pub use recovery::{recover_open_sessions, RecoveryReport};

use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("fact needs an open session but none is open")]
    NoOpenSession,
    #[error("fact needs an open block but none is open")]
    NoOpenBlock,
    #[error(
        "the database was written by a newer version (schema {found}, this build supports {supported})"
    )]
    NewerSchema { found: i64, supported: i64 },
}

pub fn open_db(path: &Path) -> Result<Connection, StoreError> {
    prepare(Connection::open(path)?)
}

pub fn open_in_memory() -> Result<Connection, StoreError> {
    prepare(Connection::open_in_memory()?)
}

fn prepare(mut conn: Connection) -> Result<Connection, StoreError> {
    conn.busy_timeout(Duration::from_secs(5))?;
    let found = migrations::user_version(&conn)?;
    if found > migrations::SCHEMA_VERSION {
        return Err(StoreError::NewerSchema {
            found,
            supported: migrations::SCHEMA_VERSION,
        });
    }
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
    migrations::migrate(&mut conn)?;
    Ok(conn)
}

/// The version a populated database will be migrated from, so the caller can back it up first.
/// `None` for fresh, empty or current databases.
pub fn pending_upgrade(conn: &Connection) -> Result<Option<i64>, StoreError> {
    let version = migrations::user_version(conn)?;
    if version == 0 || version >= migrations::SCHEMA_VERSION {
        return Ok(None);
    }
    let has_sessions: bool =
        conn.query_row("SELECT EXISTS (SELECT 1 FROM session)", [], |r| r.get(0))?;
    Ok(has_sessions.then_some(version))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(conn: &Connection, sql: &str) -> i64 {
        conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn migrations_are_idempotent_and_seed_packs() {
        let mut conn = open_in_memory().unwrap();
        migrations::migrate(&mut conn).unwrap();
        assert_eq!(
            count(&conn, "PRAGMA user_version"),
            migrations::SCHEMA_VERSION
        );
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM pack"), 11);
    }

    #[test]
    fn foreign_keys_are_enforced() {
        let conn = open_in_memory().unwrap();
        let orphan = conn.execute(
            "INSERT INTO block (session_id, kind, started_at) VALUES (999, 'freeplay', 0)",
            [],
        );
        assert!(orphan.is_err());
    }

    #[test]
    fn check_constraints_reject_bad_values() {
        let conn = open_in_memory().unwrap();
        let bad_mmr = conn.execute(
            "INSERT INTO mmr_entry (team_size, mmr, recorded_at) VALUES (2, 5000, 0)",
            [],
        );
        assert!(bad_mmr.is_err());
        let bad_size = conn.execute(
            "INSERT INTO mmr_entry (team_size, mmr, recorded_at) VALUES (4, 900, 0)",
            [],
        );
        assert!(bad_size.is_err());
    }

    #[test]
    fn file_database_persists_between_opens() {
        let dir = std::env::temp_dir().join(format!("rl-core-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("db.sqlite");
        {
            let conn = open_db(&path).unwrap();
            conn.execute("INSERT INTO session (started_at) VALUES (1)", [])
                .unwrap();
        }
        let conn = open_db(&path).unwrap();
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM session"), 1);
        drop(conn);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn v1_db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        migrations::migrate_to(&mut conn, 1).unwrap();
        conn
    }

    #[test]
    fn populated_v1_database_upgrades_and_keeps_the_earliest_profile_active() {
        let mut conn = v1_db();
        conn.execute_batch(
            "INSERT INTO profile VALUES ('Epic|b|0', 'B', 2);
             INSERT INTO profile VALUES ('Epic|a|0', 'A', 1);
             INSERT INTO session (started_at) VALUES (5);",
        )
        .unwrap();
        assert_eq!(pending_upgrade(&conn).unwrap(), Some(1));
        migrations::migrate(&mut conn).unwrap();
        assert_eq!(pending_upgrade(&conn).unwrap(), None);
        assert_eq!(
            profile_primary_id(&conn).unwrap().as_deref(),
            Some("Epic|a|0")
        );
        let last_seen: Option<i64> = conn
            .query_row("SELECT last_seen_at FROM session", [], |r| r.get(0))
            .unwrap();
        assert_eq!(last_seen, None);
        conn.execute("INSERT INTO setting (key, value) VALUES ('k', 'v')", [])
            .unwrap();
    }

    #[test]
    fn empty_v1_database_needs_no_pre_migration_backup() {
        assert_eq!(pending_upgrade(&v1_db()).unwrap(), None);
    }

    #[test]
    fn a_database_from_a_newer_build_is_refused() {
        let dir = std::env::temp_dir().join(format!("rl-core-newer-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("db.sqlite");
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "user_version", migrations::SCHEMA_VERSION + 1)
                .unwrap();
        }
        let err = open_db(&path).unwrap_err();
        assert!(matches!(
            err,
            StoreError::NewerSchema { found, supported }
                if found == migrations::SCHEMA_VERSION + 1 && supported == migrations::SCHEMA_VERSION
        ));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn exactly_one_profile_is_active() {
        let conn = open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO profile (primary_id, display_name, created_at, is_active)
                 VALUES ('Epic|a|0', 'A', 1, 1), ('Epic|b|0', 'B', 2, 0);",
        )
        .unwrap();
        let second_active = conn.execute(
            "UPDATE profile SET is_active = 1 WHERE primary_id = 'Epic|b|0'",
            [],
        );
        assert!(second_active.is_err(), "partial unique index");
        assert!(set_active_profile(&conn, "Epic|b|0").unwrap());
        assert_eq!(
            profile_primary_id(&conn).unwrap().as_deref(),
            Some("Epic|b|0")
        );
        assert!(!set_active_profile(&conn, "Epic|zzz|0").unwrap());
        let profiles = list_profiles(&conn).unwrap();
        assert_eq!(
            profiles
                .iter()
                .map(|p| (p.primary_id.as_str(), p.is_active))
                .collect::<Vec<_>>(),
            [("Epic|a|0", false), ("Epic|b|0", true)]
        );
    }

    #[test]
    fn version_3_upgrades_abandoned_attempts_and_marks_closed_sessions_reviewed() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrations::migrate_to(&mut conn, 3).unwrap();
        conn.execute_batch(
            "INSERT INTO session (id, started_at, ended_at) VALUES (1, 0, 100), (2, 200, NULL);
             INSERT INTO block (id, session_id, kind, started_at) VALUES
                (1, 1, 'training', 0), (2, 2, 'training', 200);
             INSERT INTO training_attempt (block_id, seq, started_at, duration_ms, outcome) VALUES
                (1, 1, 0, 900, 'abandoned'), (1, 2, 0, 900, 'make'), (2, 1, 200, 900, 'abandoned');",
        )
        .unwrap();
        migrations::migrate(&mut conn).unwrap();
        assert_eq!(
            count(
                &conn,
                "SELECT COUNT(*) FROM training_attempt WHERE outcome = 'abandoned'"
            ),
            0
        );
        assert_eq!(
            count(
                &conn,
                "SELECT COUNT(*) FROM training_attempt WHERE outcome = 'miss'"
            ),
            2
        );
        assert_eq!(
            count(&conn, "SELECT reviewed_at FROM session WHERE id = 1"),
            100
        );
        assert_eq!(
            count(
                &conn,
                "SELECT COUNT(*) FROM session WHERE id = 2 AND reviewed_at IS NULL"
            ),
            1,
            "a crashed (still open) session must still be offered for review after recovery"
        );
    }

    #[test]
    fn version_5_moves_results_off_between_games_blocks_and_scores_unknown_results() {
        // 2026-10-08: private 1v1s left each result on a 3-second block before the real game,
        // and a forfeited ranked 2v2 (5–6) was stored with no result.
        let mut conn = Connection::open_in_memory().unwrap();
        migrations::migrate_to(&mut conn, 5).unwrap();
        conn.execute_batch(
            "INSERT INTO session (id, started_at, ended_at) VALUES (1, 0, 2000000);
             INSERT INTO block (id, session_id, kind, playlist_id, started_at, ended_at) VALUES
                (24, 1, 'match', 6, 0, 400000), (25, 1, 'match', 6, 400000, 403000),
                (26, 1, 'match', 6, 405000, 800000), (37, 1, 'match', 11, 900000, 1400000),
                (38, 1, 'match', 11, 1500000, 1900000);
             INSERT INTO match_record (block_id, match_guid, playlist_id, ranked, team_size, arena,
                 result, team_score, opponent_score, overtime, score, goals, assists, saves, shots,
                 touches, demos, epic_saves, avg_boost, pct_zero_boost, pct_supersonic,
                 metrics_partial, scoreboard_json) VALUES
                (24, 'A', 6, 0, 1, 'woods_p', 'loss', 3, 5, 0, 0, 0, 0, 0, 0, 90, 0, 0, 0, 0, 0, 0, '[]'),
                (25, 'B', 6, 0, 1, 'woods_p', 'win', 7, 5, 0, 0, 0, 0, 0, 0, 90, 0, 0, 0, 0, 0, 1, '[]'),
                (37, 'C', 11, 1, 2, 'cs_day_p', 'unknown', 5, 6, 0, 0, 0, 0, 0, 0, 38, 0, 0, 0, 0, 0, 0, '[]'),
                (38, 'D', 11, 1, 2, 'Park_P', 'unknown', 2, 2, 0, 0, 0, 0, 0, 0, 30, 0, 0, 0, 0, 0, 0, '[]');",
        )
        .unwrap();
        migrations::migrate(&mut conn).unwrap();
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM block WHERE id = 25"), 0);
        assert_eq!(
            count(
                &conn,
                "SELECT block_id FROM match_record WHERE match_guid = 'B'"
            ),
            26
        );
        assert_eq!(
            count(
                &conn,
                "SELECT block_id FROM match_record WHERE match_guid = 'A'"
            ),
            24,
            "a real game keeps its record"
        );
        let result = |guid: &str| -> String {
            conn.query_row(
                "SELECT result FROM match_record WHERE match_guid = ?1",
                [guid],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(result("C"), "loss", "behind 5–6 when it ended");
        assert_eq!(result("D"), "unknown", "level");
    }

    #[test]
    fn migration_7_leaves_older_matches_unrecorded() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrations::migrate_to(&mut conn, 6).unwrap();
        conn.execute_batch(
            "INSERT INTO session (id, started_at) VALUES (1, 0);
             INSERT INTO block (id, session_id, kind, started_at) VALUES (1, 1, 'match', 0);
             INSERT INTO match_record (block_id, match_guid, ranked, arena, result, team_score,
                 opponent_score, overtime, score, goals, assists, saves, shots, touches, demos,
                 epic_saves, avg_boost, pct_zero_boost, pct_supersonic, metrics_partial,
                 scoreboard_json)
             VALUES (1, 'G', 1, 'Stadium_P', 'win', 1, 0, 0, 100, 1, 0, 0, 2, 10, 0, 0, 30, 10, 12,
                 0, '[]');",
        )
        .unwrap();
        migrations::migrate(&mut conn).unwrap();
        let row: (i64, Option<f64>, Option<i64>) = conn
            .query_row(
                "SELECT score, play_seconds, big_pads FROM match_record",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(row, (100, None, None));
    }

    #[test]
    fn migration_14_skips_the_automatic_import_only_for_logs_that_had_games() {
        let done = |conn: &Connection| {
            count(
                conn,
                "SELECT COUNT(*) FROM setting WHERE key = 'backfill_done' AND value = 'true'",
            )
        };
        let mut fresh = Connection::open_in_memory().unwrap();
        migrations::migrate(&mut fresh).unwrap();
        assert_eq!(done(&fresh), 0, "a new install keeps the automatic import");

        let mut played = Connection::open_in_memory().unwrap();
        migrations::migrate_to(&mut played, 13).unwrap();
        played
            .execute_batch(
                "INSERT INTO session (id, started_at) VALUES (1, 0);
                 INSERT INTO block (id, session_id, kind, started_at) VALUES (1, 1, 'match', 0);
                 INSERT INTO match_record (block_id, match_guid, ranked, arena, result, team_score,
                    opponent_score, overtime, score, goals, assists, saves, shots, touches, demos,
                    epic_saves, avg_boost, pct_zero_boost, pct_supersonic, metrics_partial,
                    scoreboard_json)
                 VALUES (1, 'G1', 1, 'Stadium_P', 'win', 3, 1, 0, 400, 2, 0, 1, 3, 20, 0, 0,
                    50, 5, 10, 0, '[]');",
            )
            .unwrap();
        migrations::migrate(&mut played).unwrap();
        assert_eq!(done(&played), 1);

        let mut emptied = Connection::open_in_memory().unwrap();
        migrations::migrate_to(&mut emptied, 13).unwrap();
        emptied
            .execute("INSERT INTO deleted_match (started_at) VALUES (5)", [])
            .unwrap();
        migrations::migrate(&mut emptied).unwrap();
        assert_eq!(
            done(&emptied),
            1,
            "everything deleted still counts as an upgrade"
        );
    }

    #[test]
    fn migration_8_makes_every_existing_pack_a_striker_pack() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrations::migrate_to(&mut conn, 7).unwrap();
        migrations::migrate(&mut conn).unwrap();
        let goalie: i64 = conn
            .query_row("SELECT COUNT(*) FROM pack WHERE goalie != 0", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(goalie, 0);
    }
}
