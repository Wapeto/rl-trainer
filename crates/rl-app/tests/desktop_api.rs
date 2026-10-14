mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use rl_app::testing::FakePlatform;
use rl_app::App;
use serde_json::{json, Value};
use tower::ServiceExt;

async fn post(
    app: &App,
    command: &str,
    body: Value,
    host: &str,
    marked: bool,
) -> (StatusCode, Value) {
    let mut req = Request::post(format!("/desktop-api/{command}"))
        .header("host", host)
        .header("content-type", "application/json");
    if marked {
        req = req.header("x-rl-desktop", "1");
    }
    let res = rl_app::desktop::http::router(app.clone())
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn the_shim_only_answers_marked_requests_to_this_pc() {
    let dir = tempfile::tempdir().unwrap();
    let app = start(&dir, FakePlatform::new(), None).await;
    let (unmarked, _) = post(&app, "about", Value::Null, "127.0.0.1:5174", false).await;
    assert_eq!(unmarked, StatusCode::FORBIDDEN);
    let (rebound, _) = post(&app, "about", Value::Null, "evil.example:5174", true).await;
    assert_eq!(
        rebound,
        StatusCode::FORBIDDEN,
        "a DNS-rebound page cannot call in"
    );
    let (ok, body) = post(&app, "about", Value::Null, "localhost:5174", true).await;
    assert_eq!(ok, StatusCode::OK);
    assert_eq!(body["ok"], true);
    assert_eq!(body["data"]["version"], env!("CARGO_PKG_VERSION"));
    app.shutdown().await;
}

#[tokio::test]
async fn commands_run_through_the_dispatcher_and_errors_are_readable() {
    let dir = tempfile::tempdir().unwrap();
    let app = start(&dir, FakePlatform::new(), None).await;
    let host = "127.0.0.1:49322";
    let (_, sessions) = post(
        &app,
        "sessions",
        json!({"search": "", "offset": 0, "limit": 50}),
        host,
        true,
    )
    .await;
    assert_eq!(sessions["data"], json!([]));
    let (status, unknown) = post(&app, "nope", Value::Null, host, true).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(unknown["error"]["message"], "unknown command: nope");
    let (_, bad) = post(&app, "sessions", json!({}), host, true).await;
    assert!(bad["error"]["message"]
        .as_str()
        .unwrap()
        .starts_with("bad arguments"));
    let pack = json!({"code": "1111222233334444", "name": "Fresh", "target_rate": 0.65,
                      "shot_count": null, "tags": [], "notes": ""});
    let (_, added) = post(&app, "add_pack", json!({"pack": pack}), host, true).await;
    assert!(added["data"].is_i64(), "{added}");
    let (_, listed) = post(
        &app,
        "packs",
        json!({"sort": "name", "search": "fresh"}),
        host,
        true,
    )
    .await;
    assert_eq!(listed["data"][0]["pack"]["code"], "1111-2222-3333-4444");
    let (_, taken) = post(&app, "add_pack", json!({"pack": pack}), host, true).await;
    assert_eq!(
        taken["error"]["message"],
        "a pack with the code 1111-2222-3333-4444 already exists"
    );
    app.shutdown().await;
}

#[tokio::test]
async fn the_desktop_page_is_served() {
    let dir = tempfile::tempdir().unwrap();
    let app = start(&dir, FakePlatform::new(), None).await;
    let res = rl_app::desktop::http::router(app.clone())
        .oneshot(
            Request::get("/")
                .header("host", "127.0.0.1:1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["content-type"], "text/html; charset=utf-8");
    app.shutdown().await;
}

#[tokio::test]
async fn progress_commands_answer_and_check_their_arguments() {
    let dir = tempfile::tempdir().unwrap();
    let app = start(&dir, FakePlatform::new(), None).await;
    let call = |command: &'static str, args: Value| {
        let app = app.clone();
        async move { rl_app::desktop::dispatch(&app, command, args).await }
    };
    let list = call("progress_list", json!({"range": "all"}))
        .await
        .unwrap();
    assert_eq!(list["sessions"], 0);
    let overview = call("progress_overview", json!({"range": "days30"}))
        .await
        .unwrap();
    assert!(overview["start"].is_number());
    let mode = call("progress_mode", json!({"team_size": 2, "range": "days90"}))
        .await
        .unwrap();
    assert_eq!(mode["stats"].as_array().unwrap().len(), 31);
    let bench = call("mode_benchmark", json!({"team_size": 2}))
        .await
        .unwrap();
    assert!(bench["matches"].is_number());
    assert_eq!(
        call("mode_benchmark", json!({"team_size": 0}))
            .await
            .unwrap_err(),
        "the team size must be 1, 2 or 3"
    );
    let pack = call("progress_pack", json!({"pack_id": 1, "range": "all"}))
        .await
        .unwrap();
    assert_eq!(pack["pack"]["id"], 1);
    assert_eq!(
        call("progress_mode", json!({"team_size": 4, "range": "all"}))
            .await
            .unwrap_err(),
        "the team size must be 1, 2 or 3"
    );
    assert!(call("progress_list", json!({"range": "week"}))
        .await
        .unwrap_err()
        .starts_with("bad arguments"));
    app.shutdown().await;
}

#[tokio::test]
async fn the_review_and_replay_commands_answer() {
    let dir = tempfile::tempdir().unwrap();
    let app = start(&dir, FakePlatform::new(), None).await;
    let call = |command: &'static str, args: Value| {
        let app = app.clone();
        async move { rl_app::desktop::dispatch(&app, command, args).await }
    };
    let review = call("session_review", json!({ "id": 1 })).await.unwrap();
    assert!(review.is_null(), "no such session");
    assert!(call("session_review", json!({})).await.is_err());
    let to_save = call("replays_to_save", Value::Null).await.unwrap();
    assert!(to_save.is_array());
    let status = call("replay_status", Value::Null).await.unwrap();
    assert_eq!(status["counts"]["imported"], 0);
    assert!(status["folders"].is_array());
    let cards = call(
        "sessions",
        json!({ "search": "", "offset": 0, "limit": 10, "to_save": true }),
    )
    .await
    .unwrap();
    assert!(cards.is_array());
    let replay = call("match_replay", json!({ "id": 1 })).await.unwrap();
    assert_eq!(replay["players"], json!([]), "no such match");
    assert_eq!(replay["benchmark"]["matches"], 0);
    assert!(replay["benchmark"]["winners"].is_null());
    assert_eq!(replay["insights"], json!([]));
    assert!(call("match_replay", json!({})).await.is_err());
    app.shutdown().await;
}
