mod common;

use std::net::SocketAddr;
use std::sync::Arc;

use common::*;
use rl_app::backup::BackupKind;
use rl_app::paths::AppPaths;
use rl_app::settings::SettingsPatch;
use rl_app::testing::FakePlatform;
use rl_app::App;
use rl_core::store::packs::{PackInput, PackSort};
use rl_core::store::sessions::Neighbour;
use rl_core::store::MmrInput;

/// Two finished, unreviewed sessions: 1 (training, match) and 2 (match 20000–25000, training).
const SEED: &str = "INSERT INTO session (id, started_at, ended_at) VALUES
        (1, 1000, 9000), (2, 20000, 30000);
    INSERT INTO block (id, session_id, kind, started_at, ended_at) VALUES
        (11, 1, 'training', 1000, 4000), (12, 1, 'match', 5000, 9000),
        (21, 2, 'match', 20000, 25000), (22, 2, 'training', 26000, 30000);";

async fn seeded(platform: Arc<FakePlatform>) -> (tempfile::TempDir, App) {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::under(&dir.path().join("app"));
    paths.ensure_dirs().unwrap();
    rl_core::store::open_db(&paths.db)
        .unwrap()
        .execute_batch(SEED)
        .unwrap();
    let app = start(&dir, platform, None).await;
    (dir, app)
}

fn pre_edit(app: &App) -> Vec<String> {
    app.list_backups()
        .unwrap()
        .into_iter()
        .filter(|b| b.kind == BackupKind::PreEdit)
        .map(|b| b.name)
        .collect()
}

#[tokio::test]
async fn a_merge_is_backed_up_first_and_a_refused_one_is_not() {
    let (_dir, app) = seeded(FakePlatform::new()).await;
    let refused = app
        .merge_sessions(1, Neighbour::Previous)
        .await
        .unwrap_err();
    assert_eq!(refused.to_string(), "there is no earlier session");
    assert!(pre_edit(&app).is_empty(), "no backup for a refused edit");
    assert_eq!(app.merge_sessions(2, Neighbour::Previous).await.unwrap(), 1);
    let backups = pre_edit(&app);
    assert_eq!(backups.len(), 1);
    assert!(backups[0].ends_with("-merge.db"), "{backups:?}");
    assert_eq!(app.session_cards("", 0, 50).unwrap().len(), 1);
    app.shutdown().await;
}

#[tokio::test]
async fn only_the_last_three_before_edit_backups_are_kept() {
    let (_dir, app) = seeded(FakePlatform::new()).await;
    let first = app.split_session(2, 21).await.unwrap();
    assert_eq!(
        app.merge_sessions(first, Neighbour::Previous)
            .await
            .unwrap(),
        2
    );
    let second = app.split_session(2, 21).await.unwrap();
    app.delete_session(second).await.unwrap();
    assert_eq!(pre_edit(&app).len(), 3, "four edits, three copies kept");
    assert_eq!(app.session_cards("", 0, 50).unwrap().len(), 2);
    assert!(
        app.delete_activity(21).await.unwrap(),
        "21 was session 2's only activity"
    );
    assert_eq!(app.session_cards("", 0, 50).unwrap().len(), 1);
    app.shutdown().await;
}

#[tokio::test]
async fn a_review_edit_shows_in_the_detail_and_the_count() {
    let (_dir, app) = seeded(FakePlatform::new()).await;
    assert_eq!(app.review_count().unwrap(), 2);
    app.edit_review(
        1,
        vec![MmrInput {
            team_size: 2,
            mmr: 910,
        }],
        "fine".into(),
    )
    .await
    .unwrap();
    let d = app.session_detail(1).unwrap().unwrap();
    assert_eq!(
        (d.notes.as_str(), d.mmr.len(), d.needs_review),
        ("fine", 1, false)
    );
    assert_eq!(app.review_count().unwrap(), 1);
    app.shutdown().await;
}

#[tokio::test]
async fn packs_added_and_hidden_on_the_desktop_reach_the_phone_list() {
    let (_dir, app) = seeded(FakePlatform::new()).await;
    let input = PackInput {
        code: "1111 2222 3333 4444".into(),
        name: "Fresh".into(),
        target_rate: 0.7,
        shot_count: None,
        tags: Vec::new(),
        notes: String::new(),
        goalie: false,
    };
    let id = app.add_pack(input.clone()).await.unwrap();
    assert!(app.packs("").unwrap().iter().any(|p| p.pack.id == id));
    app.set_pack_hidden(id, true).await.unwrap();
    assert!(
        app.packs("").unwrap().iter().all(|p| p.pack.id != id),
        "the phone no longer lists it"
    );
    let listed = app.pack_list(PackSort::Name, "fresh").unwrap();
    assert!(listed[0].pack.hidden, "the desktop still does");
    let renamed = PackInput {
        name: "Fresher".into(),
        ..input
    };
    app.update_pack(id, renamed).await.unwrap();
    assert_eq!(
        app.pack_detail(id).unwrap().unwrap().stats.pack.name,
        "Fresher"
    );
    app.shutdown().await;
}

#[tokio::test]
async fn a_new_port_restarts_the_running_phone_server() {
    let (_dir, app) = seeded(FakePlatform::new()).await;
    app.enable_phone_at(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let free = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let patch = SettingsPatch {
        phone_port: Some(free),
        ..Default::default()
    };
    assert_eq!(app.update_settings(&patch).await.unwrap().phone_port, free);
    assert_eq!(
        app.pairing().await.unwrap().port,
        free,
        "restarted on the new port"
    );
    app.shutdown().await;
}

#[tokio::test]
async fn open_releases_opens_the_releases_page() {
    let platform = FakePlatform::new();
    let (_dir, app) = seeded(platform.clone()).await;
    app.open_releases().unwrap();
    assert_eq!(
        platform.opened_folders(),
        [std::path::PathBuf::from(rl_app::RELEASES_URL)]
    );
    app.shutdown().await;
}

#[tokio::test]
async fn open_data_folder_opens_the_app_folder() {
    let platform = FakePlatform::new();
    let (_dir, app) = seeded(platform.clone()).await;
    app.open_data_folder().unwrap();
    assert_eq!(platform.opened_folders(), [app.paths().root.clone()]);
    app.shutdown().await;
}

#[tokio::test]
async fn the_phone_pack_list_searches_in_rust() {
    let (_dir, app) = seeded(FakePlatform::new()).await;
    let found = app.packs("5cb26d82").unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].pack.code, "5CB2-6D82-1B54-47B7");
    assert!(app.packs("no such pack").unwrap().is_empty());
    app.shutdown().await;
}

#[tokio::test]
async fn a_port_that_cannot_be_used_keeps_the_old_one() {
    let (_dir, app) = seeded(FakePlatform::new()).await;
    let before = app
        .enable_phone_at(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap()
        .port;
    let saved = app.settings().unwrap().phone_port;
    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let busy = taken.local_addr().unwrap().port();
    let patch = SettingsPatch {
        phone_port: Some(busy),
        ..Default::default()
    };
    let err = app.update_settings(&patch).await.unwrap_err().to_string();
    assert!(err.contains("cannot listen"), "{err}");
    assert_eq!(
        app.settings().unwrap().phone_port,
        saved,
        "the old port is saved again"
    );
    assert_eq!(
        app.pairing().await.expect("still serving").port,
        before,
        "the phone page runs on the old port"
    );
    drop(taken);
    app.shutdown().await;
}

#[tokio::test]
async fn a_failed_port_change_puts_the_firewall_rule_back_for_the_old_port() {
    let platform = FakePlatform::new();
    let (_dir, app) = seeded(platform.clone()).await;
    let old = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let set_port = |port: u16| SettingsPatch {
        phone_port: Some(port),
        ..Default::default()
    };
    app.update_settings(&set_port(old)).await.unwrap();
    // The LAN server (FakePlatform serves it on loopback, with a recorded firewall rule).
    app.enable_phone().await.unwrap();
    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let busy = taken.local_addr().unwrap().port();
    assert!(app.update_settings(&set_port(busy)).await.is_err());
    let rules: Vec<String> = platform
        .elevated_calls()
        .iter()
        .filter(|c| c.get(1).is_some_and(|a| a == "add-firewall-rule"))
        .map(|c| c[3].clone())
        .collect();
    assert_eq!(
        rules.last(),
        Some(&old.to_string()),
        "the only firewall rule must allow the port the phone page listens on: {rules:?}"
    );
    assert_eq!(app.settings().unwrap().firewall_port, Some(old));
    assert_eq!(app.pairing().await.expect("still serving").port, old);
    drop(taken);
    app.shutdown().await;
}

#[tokio::test]
async fn a_failed_port_change_undoes_the_whole_patch() {
    let (_dir, app) = seeded(FakePlatform::new()).await;
    app.enable_phone_at(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let before = app.settings().unwrap();
    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let patch = SettingsPatch {
        phone_port: Some(taken.local_addr().unwrap().port()),
        hard_hit_kmh: Some(95.0),
        ..Default::default()
    };
    assert!(app.update_settings(&patch).await.is_err());
    let after = app.settings().unwrap();
    assert_eq!(
        after.hard_hit_kmh, before.hard_hit_kmh,
        "nothing of the failed patch stays"
    );
    assert_eq!(after.phone_port, before.phone_port);
    assert!(app.pairing().await.is_some(), "the phone page still runs");
    drop(taken);
    app.shutdown().await;
}

#[tokio::test]
async fn saving_an_account_hint_activates_the_matching_profile_or_clears_the_flag() {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::under(&dir.path().join("app"));
    paths.ensure_dirs().unwrap();
    rl_core::store::open_db(&paths.db)
        .unwrap()
        .execute_batch(
            "INSERT INTO profile (primary_id, display_name, created_at, is_active)
             VALUES ('Epic|aaa|0', 'Alpha', 1, 1), ('Epic|bbb|0', 'Beta', 2, 0);",
        )
        .unwrap();
    let app = start(&dir, FakePlatform::new(), None).await;
    let hint = |text: &str| SettingsPatch {
        me_hint: Some(text.into()),
        ..Default::default()
    };
    let active = |app: &App| {
        app.profiles()
            .unwrap()
            .into_iter()
            .find(|p| p.is_active)
            .map(|p| p.primary_id)
    };
    app.update_settings(&hint("beta")).await.unwrap();
    assert_eq!(active(&app).as_deref(), Some("Epic|bbb|0"));
    app.update_settings(&hint("Somebody Else")).await.unwrap();
    assert_eq!(active(&app), None, "no match: detect again next start");
    app.update_settings(&hint("beta")).await.unwrap();
    app.update_settings(&hint("")).await.unwrap();
    assert_eq!(
        active(&app).as_deref(),
        Some("Epic|bbb|0"),
        "clearing keeps the account"
    );
    app.shutdown().await;
}
