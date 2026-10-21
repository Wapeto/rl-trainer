//! Past games from replay files (v0.3.0 spec §2): the replay's score replaces the live one, and
//! replays of games never recorded become sessions of their own.

mod common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use rl_app::replays::extract::{human_keys, read_guid};
use rl_app::testing::FakePlatform;
use rl_app::{App, AppConfig};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/match-2026-10.replay"
);

/// An online replay; not committed, so these tests skip with a note without it.
fn fixture() -> Option<Vec<u8>> {
    let bytes = std::fs::read(FIXTURE).ok();
    if bytes.is_none() {
        eprintln!("skipped: no replay fixture at {FIXTURE}");
    }
    bytes
}

struct Setup {
    _dir: tempfile::TempDir,
    config: AppConfig,
    guid: String,
}

/// An app config watching a folder that holds the fixture, and a profile for its first player.
/// `first_start_done` keeps the start from importing by itself.
fn setup(bytes: &[u8], first_start_done: bool) -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let replays = dir.path().join("replays");
    std::fs::create_dir_all(&replays).unwrap();
    std::fs::write(replays.join("match.replay"), bytes).unwrap();
    let me = human_keys(bytes).unwrap().remove(0).primary_id();
    let config = AppConfig {
        replay_dir: Some(replays),
        replay_poll_every: Duration::from_secs(3600),
        watch_game: false,
        ..common::config(&dir, None)
    };
    std::fs::create_dir_all(config.paths.db.parent().unwrap()).unwrap();
    let conn = rl_core::store::open_db(&config.paths.db).unwrap();
    conn.execute(
        "INSERT INTO profile (primary_id, display_name, created_at, is_active)
         VALUES (?1, 'me', 0, 1)",
        [&me],
    )
    .unwrap();
    if first_start_done {
        conn.execute(
            "INSERT INTO setting (key, value) VALUES ('backfill_done', 'true')",
            [],
        )
        .unwrap();
    }
    Setup {
        _dir: dir,
        config,
        guid: read_guid(bytes).unwrap().unwrap(),
    }
}

fn count(db: &Path, sql: &str) -> i64 {
    let conn = rl_core::store::open_db(db).unwrap();
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

fn db_of(s: &Setup) -> PathBuf {
    s.config.paths.db.clone()
}

#[tokio::test]
async fn a_past_game_becomes_a_replay_session_once() {
    let Some(bytes) = fixture() else { return };
    let s = setup(&bytes, true);
    let db = db_of(&s);
    let app = App::start(s.config.clone(), FakePlatform::new())
        .await
        .unwrap();
    app.import_past_games(20).await.unwrap();
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM session WHERE source = 'replay'"),
        1
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM match_record"), 1);
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM match_record WHERE result_source = 'replay' AND ranked = 0"
        ),
        1
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM replay_player"), 4);
    assert_eq!(app.import_past_games(20).await.unwrap(), 0, "nothing new");
    assert_eq!(count(&db, "SELECT COUNT(*) FROM match_record"), 1);
    app.shutdown().await;
}

#[tokio::test]
async fn the_limit_is_respected() {
    let Some(bytes) = fixture() else { return };
    let s = setup(&bytes, true);
    let db = db_of(&s);
    let app = App::start(s.config.clone(), FakePlatform::new())
        .await
        .unwrap();
    assert_eq!(app.import_past_games(0).await.unwrap(), 0);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM match_record"), 0);
    app.shutdown().await;
}

#[tokio::test]
async fn a_game_already_in_the_log_is_not_added_again() {
    let Some(bytes) = fixture() else { return };
    let s = setup(&bytes, true);
    let db = db_of(&s);
    let conn = rl_core::store::open_db(&db).unwrap();
    conn.execute_batch(&format!(
        "INSERT INTO session (id, started_at, ended_at) VALUES (1, 0, 600000);
         INSERT INTO block (id, session_id, kind, started_at, ended_at) VALUES (1, 1, 'match', 0, 300000);
         INSERT INTO match_record (id, block_id, match_guid, playlist_id, ranked, team_size, arena,
             result, team_score, opponent_score, overtime, score, goals, assists, saves, shots,
             touches, demos, epic_saves, avg_boost, pct_zero_boost, pct_supersonic,
             metrics_partial, scoreboard_json)
         VALUES (1, 1, '{}', 11, 1, 2, 'Park_P', 'win', 3, 1, 0, 300, 1, 0, 1, 3, 30, 0, 0,
             40, 15, 10, 0, '[]');",
        s.guid
    ))
    .unwrap();
    drop(conn);
    let app = App::start(s.config.clone(), FakePlatform::new())
        .await
        .unwrap();
    assert_eq!(app.import_past_games(20).await.unwrap(), 0);
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM session WHERE source = 'replay'"),
        0
    );
    app.shutdown().await;
}

#[tokio::test]
async fn the_first_start_imports_once() {
    let Some(bytes) = fixture() else { return };
    let s = setup(&bytes, false);
    let db = db_of(&s);
    let app = App::start(s.config.clone(), FakePlatform::new())
        .await
        .unwrap();
    for _ in 0..100 {
        if count(&db, "SELECT COUNT(*) FROM match_record") == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(count(&db, "SELECT COUNT(*) FROM match_record"), 1);
    app.shutdown().await;
    // The user deletes the game; a later start must not bring it back.
    let conn = rl_core::store::open_db(&db).unwrap();
    conn.execute_batch(
        "DELETE FROM match_record; DELETE FROM block; DELETE FROM session;
         DELETE FROM replay_file;",
    )
    .unwrap();
    drop(conn);
    let app = App::start(s.config.clone(), FakePlatform::new())
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(count(&db, "SELECT COUNT(*) FROM match_record"), 0);
    app.shutdown().await;
}
