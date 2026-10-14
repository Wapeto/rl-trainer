mod common;

use std::time::Duration;

use common::*;
use rl_app::devtools::{update_state_json, FakeStatsApi};
use rl_app::game_watch::ini::StatsApiStatus;
use rl_app::paths::AppPaths;
use rl_app::platform::PlatformError;
use rl_app::testing::{ElevationMode, FakePlatform};
use rl_app::{App, AppConfig, AppError};

async fn freeplay_api() -> FakeStatsApi {
    FakeStatsApi::start(
        vec![],
        Some((Duration::from_millis(50), update_state_json(9, 10.0))),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn game_start_checks_the_ini_and_records_through_its_port() {
    let dir = tempfile::tempdir().unwrap();
    let api = freeplay_api().await;
    let exe = install(dir.path(), &ini_text(10, port_of(&api.addr)));
    let platform = FakePlatform::new();
    platform.set_game(game(&exe));
    let app = start(&dir, platform.clone(), None).await;
    let s = wait_for(&app, "recording", |s| {
        matches!(s.stats_api, StatsApiStatus::Ok { .. }) && s.live.block.is_some()
    })
    .await;
    assert_eq!(s.stats_api.port(), Some(port_of(&api.addr)));
    platform.set_game(None);
    wait_for(&app, "the session to close", |s| {
        !s.game_running && s.live.last_session.is_some()
    })
    .await;
    app.shutdown().await;
}

#[tokio::test]
async fn fix_runs_the_elevated_command_and_rechecks() {
    let dir = tempfile::tempdir().unwrap();
    let exe = install(dir.path(), &ini_text(0, 49_123));
    let platform = FakePlatform::new();
    platform.set_game(game(&exe));
    let app = start(&dir, platform.clone(), None).await;
    wait_for(&app, "the disabled status", |s| {
        matches!(s.stats_api, StatsApiStatus::Disabled { .. })
    })
    .await;
    let status = app.fix_stats_api().await.unwrap();
    assert!(matches!(status, StatsApiStatus::Ok { rate, .. } if rate == 10.0));
    assert_eq!(platform.elevated_calls()[0][1], "fix-stats-api");
    let bak = ini_path(dir.path()).with_extension("ini.bak");
    assert!(std::fs::read_to_string(bak)
        .unwrap()
        .contains("PacketSendRate=0"));
    app.shutdown().await;
}

#[tokio::test]
async fn a_declined_prompt_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let exe = install(dir.path(), &ini_text(0, 49_123));
    let platform = FakePlatform::new();
    platform.set_game(game(&exe));
    platform.set_elevation(ElevationMode::Decline);
    let app = start(&dir, platform, None).await;
    wait_for(&app, "the disabled status", |s| {
        matches!(s.stats_api, StatsApiStatus::Disabled { .. })
    })
    .await;
    assert!(matches!(
        app.fix_stats_api().await,
        Err(AppError::Platform(PlatformError::Cancelled))
    ));
    assert!(std::fs::read_to_string(ini_path(dir.path()))
        .unwrap()
        .contains("PacketSendRate=0"));
    app.shutdown().await;
}

#[tokio::test]
async fn restore_replaces_the_database_and_jumps_the_version() {
    let dir = tempfile::tempdir().unwrap();
    let app = start(&dir, FakePlatform::new(), None).await;
    app.add_mmr(2, 850, None).await.unwrap();
    let before = wait_for(&app, "the MMR", |s| {
        s.live.latest_mmr.iter().any(|m| m.mmr == 850)
    })
    .await;
    let daily = app
        .list_backups()
        .unwrap()
        .into_iter()
        .find(|b| b.name.starts_with("rl-trainer-"))
        .expect("startup wrote today's backup");
    assert!(matches!(
        app.restore_backup("nope.db").await,
        Err(AppError::UnknownBackup(_))
    ));
    app.restore_backup(&daily.name).await.unwrap();
    let after = wait_for(&app, "the restored state", |s| {
        s.version >= before.version + 1000
    })
    .await;
    assert!(
        after.live.latest_mmr.is_empty(),
        "the backup predates the MMR entry"
    );
    assert!(app
        .list_backups()
        .unwrap()
        .iter()
        .any(|b| b.name.starts_with("pre-restore-")));
    app.add_mmr(2, 860, None).await.unwrap(); // recording continues on the restored database
    app.shutdown().await;
}

#[tokio::test]
async fn startup_imports_the_dev_db_and_recovers_its_open_session() {
    let dir = tempfile::tempdir().unwrap();
    let dev = dir.path().join("rl-trainer-dev.db");
    {
        let conn = rl_core::store::open_db(&dev).unwrap();
        conn.execute_batch(
            "INSERT INTO session (id, started_at) VALUES (1, 100);
             INSERT INTO block (session_id, kind, started_at) VALUES (1, 'freeplay', 200);",
        )
        .unwrap();
    }
    let config = AppConfig {
        dev_db: Some(dev),
        ..AppConfig::new(AppPaths::under(&dir.path().join("app")))
    };
    let app = App::start(config, FakePlatform::new()).await.unwrap();
    let report = app.startup_report();
    assert!(report.imported_dev_db);
    assert_eq!(report.sessions_recovered, 1);
    assert!(app.settings().unwrap().dev_db_imported_at.is_some());
    app.shutdown().await;
}
