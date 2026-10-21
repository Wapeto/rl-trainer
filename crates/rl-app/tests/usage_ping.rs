//! The daily install ping: sent once, not again within a day, and never when switched off.

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::Router;
use common::config;
use rl_app::testing::FakePlatform;
use rl_app::App;
use serde_json::Value;

type Seen = Arc<Mutex<Vec<(HeaderMap, Bytes)>>>;

/// A stand-in relay answering every ping with 204; returns its report address.
async fn stub() -> (String, Seen) {
    let seen = Seen::default();
    let app = Router::new()
        .route(
            "/ping",
            post(
                |State(seen): State<Seen>, h: HeaderMap, b: Bytes| async move {
                    seen.lock().unwrap().push((h, b));
                    StatusCode::NO_CONTENT
                },
            ),
        )
        .with_state(seen.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/report", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, seen)
}

/// A database that already exists, with the given settings rows.
fn seed(dir: &tempfile::TempDir, rows: &[(&str, &str)]) {
    let cfg = config(dir, None);
    std::fs::create_dir_all(cfg.paths.db.parent().unwrap()).unwrap();
    let conn = rl_core::store::open_db(&cfg.paths.db).unwrap();
    for (key, value) in rows {
        conn.execute(
            "INSERT INTO setting (key, value) VALUES (?1, ?2)",
            [key, value],
        )
        .unwrap();
    }
}

async fn start(dir: &tempfile::TempDir, url: &str) -> App {
    let mut cfg = config(dir, None);
    cfg.report_url = Some(url.to_owned());
    App::start(cfg, FakePlatform::new()).await.unwrap()
}

#[tokio::test]
async fn it_pings_once_and_not_again_within_a_day() {
    let (url, seen) = stub().await;
    let dir = tempfile::tempdir().unwrap();
    let app = start(&dir, &url).await;
    // A debug build does not ping at start by itself, so ask for it.
    assert!(app.send_usage_ping().await.unwrap());
    assert!(
        !app.send_usage_ping().await.unwrap(),
        "already pinged today"
    );
    tokio::time::sleep(Duration::from_millis(200)).await;
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    let (headers, body) = &seen[0];
    assert!(headers.contains_key("x-rl-trainer-version"));
    let body: Value = serde_json::from_slice(body).unwrap();
    let id = body["id"].as_str().unwrap();
    assert_eq!(id.len(), 32);
    assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    drop(seen);
    app.shutdown().await;
}

#[tokio::test]
async fn it_pings_again_after_a_day_with_the_same_id() {
    let (url, seen) = stub().await;
    let dir = tempfile::tempdir().unwrap();
    let app = start(&dir, &url).await;
    assert!(app.send_usage_ping().await.unwrap());
    app.shutdown().await;
    let conn = rl_core::store::open_db(&config(&dir, None).paths.db).unwrap();
    conn.execute(
        "UPDATE setting SET value = '0' WHERE key = 'last_ping_at'",
        [],
    )
    .unwrap();
    drop(conn);
    let app = start(&dir, &url).await;
    assert!(app.send_usage_ping().await.unwrap());
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].1, seen[1].1, "the same id and version");
    drop(seen);
    app.shutdown().await;
}

#[tokio::test]
async fn it_does_not_ping_when_switched_off() {
    let (url, seen) = stub().await;
    let dir = tempfile::tempdir().unwrap();
    seed(&dir, &[("usage_ping", "false")]);
    let app = start(&dir, &url).await;
    assert!(!app.send_usage_ping().await.unwrap());
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(seen.lock().unwrap().is_empty());
    app.shutdown().await;
}

#[tokio::test]
async fn it_does_not_ping_without_a_relay() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = config(&dir, None);
    cfg.report_url = None;
    let app = App::start(cfg, FakePlatform::new()).await.unwrap();
    assert!(!app.send_usage_ping().await.unwrap());
    app.shutdown().await;
}
