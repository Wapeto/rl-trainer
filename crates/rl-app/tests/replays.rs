//! A replay saved in the watched folder is attached to its recorded match (spec §4).

mod common;

use std::time::Duration;

use rl_app::paths::AppPaths;
use rl_app::testing::FakePlatform;
use rl_app::{App, AppConfig};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/match-2026-10.replay"
);

/// An online replay; not committed, so the test skips with a note without it.
fn fixture() -> Option<Vec<u8>> {
    match std::fs::read(FIXTURE) {
        Ok(bytes) => Some(bytes),
        Err(_) => {
            eprintln!("skipped: no replay fixture at {FIXTURE}");
            None
        }
    }
}

/// A database holding one recorded match whose GUID is the fixture's, and `me` as the profile.
fn seed(paths: &AppPaths, guid: &str, me: &str) {
    std::fs::create_dir_all(paths.db.parent().unwrap()).unwrap();
    let conn = rl_core::store::open_db(&paths.db).unwrap();
    conn.execute_batch(&format!(
        "INSERT INTO profile (primary_id, display_name, created_at, is_active) VALUES ('{me}', 'me', 0, 1);
         INSERT INTO session (id, started_at, ended_at) VALUES (1, 0, 600000);
         INSERT INTO block (id, session_id, kind, started_at, ended_at) VALUES (1, 1, 'match', 0, 300000);
         INSERT INTO match_record (id, block_id, match_guid, playlist_id, ranked, team_size, arena,
             result, team_score, opponent_score, overtime, score, goals, assists, saves, shots,
             touches, demos, epic_saves, avg_boost, pct_zero_boost, pct_supersonic,
             metrics_partial, scoreboard_json)
         VALUES (1, 1, '{guid}', 11, 1, 2, 'Park_P', 'win', 3, 1, 0, 300, 1, 0, 1, 3, 30, 0, 0,
             40, 15, 10, 0, '[]');"
    ))
    .unwrap();
}

#[tokio::test]
async fn a_saved_replay_is_imported_and_counted_once() {
    let dir = tempfile::tempdir().unwrap();
    let replays = dir.path().join("replays");
    std::fs::create_dir_all(&replays).unwrap();
    let Some(bytes) = fixture() else { return };
    let guid = rl_app::replays::extract::read_guid(&bytes)
        .unwrap()
        .unwrap();
    let me = rl_app::replays::extract::human_keys(&bytes)
        .unwrap()
        .remove(0)
        .primary_id();
    let config = AppConfig {
        replay_dir: Some(replays.clone()),
        replay_poll_every: Duration::from_millis(50),
        watch_game: false,
        ..common::config(&dir, None)
    };
    seed(&config.paths, &guid, &me);
    let db = config.paths.db.clone();
    let app = App::start(config, FakePlatform::new()).await.unwrap();
    std::fs::write(replays.join("match.replay"), &bytes).unwrap();
    // Backdate the file so it is old enough to be stable at once.
    let past = std::time::SystemTime::now() - Duration::from_secs(10);
    std::fs::File::options()
        .write(true)
        .open(replays.join("match.replay"))
        .unwrap()
        .set_modified(past)
        .unwrap();
    let snap = common::wait_for(&app, "the replay import", |s| s.replays >= 1).await;
    assert_eq!(snap.replays, 1);
    let detail = app.session_detail(1).unwrap().unwrap();
    let m = detail.activities[0].match_summary.as_ref().unwrap();
    assert!(m.replay.as_ref().is_some_and(|r| r.touches > 0));
    // The live record said 3-1; the replay's header has the final score.
    let conn = rl_core::store::open_db(&db).unwrap();
    let (src, mine, theirs): (String, i64, i64) = conn
        .query_row(
            "SELECT result_source, team_score, opponent_score FROM match_record WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!((src.as_str(), mine + theirs), ("replay", 3));
    drop(conn);
    let players = players(&db);
    assert_eq!(players.len(), 4, "every player of the 2v2");
    assert_eq!(players.iter().filter(|p| p.is_me).count(), 1);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        app.snapshot().replays,
        1,
        "the same file is not imported twice"
    );
    app.shutdown().await;
}

fn players(db: &std::path::Path) -> Vec<rl_core::store::replays::ReplayPlayer> {
    let conn = rl_core::store::open_db(db).unwrap();
    rl_core::store::replays::players_for_match(&conn, 1).unwrap()
}

#[tokio::test]
async fn a_replay_imported_before_the_match_page_is_read_again_once() {
    let dir = tempfile::tempdir().unwrap();
    let replays = dir.path().join("replays");
    std::fs::create_dir_all(&replays).unwrap();
    let Some(bytes) = fixture() else { return };
    let guid = rl_app::replays::extract::read_guid(&bytes)
        .unwrap()
        .unwrap();
    let me = rl_app::replays::extract::human_keys(&bytes)
        .unwrap()
        .remove(0)
        .primary_id();
    let config = AppConfig {
        replay_dir: Some(replays.clone()),
        replay_poll_every: Duration::from_millis(50),
        watch_game: false,
        ..common::config(&dir, None)
    };
    seed(&config.paths, &guid, &me);
    let file = replays.join("match.replay");
    std::fs::write(&file, &bytes).unwrap();
    let past = std::time::SystemTime::now() - Duration::from_secs(10);
    std::fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_modified(past)
        .unwrap();
    // Already imported by an older version: seen with its time and size, no player rows.
    let modified = past
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let conn = rl_core::store::open_db(&config.paths.db).unwrap();
    let rec = rl_core::store::replays::ReplayFileRecord {
        path: file.to_string_lossy().into_owned(),
        modified_at: modified,
        size: bytes.len() as i64,
        match_guid: Some(guid.clone()),
        status: rl_core::store::replays::ReplayFileStatus::Imported,
        error: None,
    };
    rl_core::store::replays::record_file(&conn, &rec, 0).unwrap();
    drop(conn);
    let db = config.paths.db.clone();
    let app = App::start(config, FakePlatform::new()).await.unwrap();
    let mut found = Vec::new();
    for _ in 0..300 {
        found = players(&db);
        if !found.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(
        found.len(),
        4,
        "the old replay was read again for every player"
    );
    app.shutdown().await;
}
