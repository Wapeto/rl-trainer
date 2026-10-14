mod common;

use std::sync::{Arc, Mutex};

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::Router;
use common::*;
use rl_app::testing::FakePlatform;
use rl_app::App;
use serde_json::{json, Value};

type Seen = Arc<Mutex<Vec<(HeaderMap, Bytes)>>>;

/// A stand-in relay answering every report with `status`.
async fn stub(status: StatusCode) -> (String, Seen) {
    let seen = Seen::default();
    let app = Router::new()
        .route(
            "/report",
            post(|State((seen, status)): State<(Seen, StatusCode)>, h: HeaderMap, b: Bytes| async move {
                seen.lock().unwrap().push((h, b));
                status
            }),
        )
        .with_state((seen.clone(), status));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/report", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, seen)
}

async fn app_with(
    dir: &tempfile::TempDir,
    url: Option<String>,
    platform: Arc<FakePlatform>,
) -> App {
    let mut cfg = config(dir, None);
    cfg.report_url = url;
    App::start(cfg, platform).await.unwrap()
}

fn args(text: &str) -> Value {
    json!({"kind": "bug", "text": text, "contact": "Sam", "screenshot": null, "include_logs": true})
}

#[tokio::test]
async fn a_report_reaches_the_relay_with_its_fields_and_version_header() {
    let (url, seen) = stub(StatusCode::OK).await;
    let dir = tempfile::tempdir().unwrap();
    let app = app_with(&dir, Some(url), FakePlatform::new()).await;
    assert_eq!(
        rl_app::desktop::dispatch(&app, "report_info", Value::Null)
            .await
            .unwrap(),
        json!({"can_send": true})
    );
    std::fs::create_dir_all(&app.paths().logs).unwrap();
    std::fs::write(app.paths().logs.join("rl.log"), "a log line").unwrap();
    rl_app::desktop::dispatch(&app, "send_report", args("the radar is empty"))
        .await
        .unwrap();
    let seen = seen.lock().unwrap();
    let (headers, body) = &seen[0];
    assert!(headers.contains_key("x-rl-trainer-version"));
    let body = String::from_utf8_lossy(body);
    for part in ["the radar is empty", "Sam", "a log line", "name=\"kind\""] {
        assert!(body.contains(part), "{part} missing");
    }
    drop(seen);
    app.shutdown().await;
}

#[tokio::test]
async fn relay_failures_become_readable_errors() {
    for (status, expect) in [
        (StatusCode::TOO_MANY_REQUESTS, "Too many reports"),
        (StatusCode::BAD_REQUEST, "refused"),
        (StatusCode::BAD_GATEWAY, "report service had a problem"),
    ] {
        let (url, _) = stub(status).await;
        let dir = tempfile::tempdir().unwrap();
        let app = app_with(&dir, Some(url), FakePlatform::new()).await;
        let err = rl_app::desktop::dispatch(&app, "send_report", args("x"))
            .await
            .unwrap_err();
        assert!(err.contains(expect), "{status}: {err}");
        app.shutdown().await;
    }
}

#[tokio::test]
async fn without_a_relay_only_saving_works_and_it_opens_the_folder() {
    let dir = tempfile::tempdir().unwrap();
    let docs = tempfile::tempdir().unwrap();
    let platform = FakePlatform::new();
    platform.set_documents(Some(docs.path().to_path_buf()));
    let app = app_with(&dir, None, platform.clone()).await;
    let info = rl_app::desktop::dispatch(&app, "report_info", Value::Null)
        .await
        .unwrap();
    assert_eq!(info, json!({"can_send": false}));
    let err = rl_app::desktop::dispatch(&app, "send_report", args("x"))
        .await
        .unwrap_err();
    assert!(err.contains("not set up"), "{err}");
    let saved = rl_app::desktop::dispatch(&app, "save_report", args("it broke"))
        .await
        .unwrap();
    let saved = std::path::PathBuf::from(saved.as_str().unwrap());
    assert!(saved.join("report.txt").exists());
    assert_eq!(platform.opened_folders(), [saved]);
    let bad = rl_app::desktop::dispatch(&app, "save_report", args("  "))
        .await
        .unwrap_err();
    assert!(bad.contains("Say what happened"), "{bad}");
    app.shutdown().await;
}
