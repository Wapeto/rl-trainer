mod common;

use std::net::SocketAddr;
use std::time::Duration;

use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{Request, StatusCode};
use common::*;
use rl_app::paths::AppPaths;
use rl_app::phone;
use rl_app::testing::FakePlatform;
use serde_json::{json, Value};
use tower::ServiceExt;

struct Phone {
    _dir: tempfile::TempDir,
    app: rl_app::App,
    base: String,
    token: String,
    http: reqwest::Client,
}

async fn phone() -> Phone {
    phone_seeded("").await
}

/// Starts the app on a database that already holds `seed` (SQL run after the migrations).
async fn phone_seeded(seed: &str) -> Phone {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::under(&dir.path().join("app"));
    paths.ensure_dirs().unwrap();
    rl_core::store::open_db(&paths.db)
        .unwrap()
        .execute_batch(seed)
        .unwrap();
    let app = start(&dir, FakePlatform::new(), None).await;
    let pairing = app
        .enable_phone_at(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let token = pairing.url.split("#t=").nth(1).unwrap().to_owned();
    Phone {
        base: format!("http://127.0.0.1:{}", pairing.port),
        token,
        http: reqwest::Client::new(),
        app,
        _dir: dir,
    }
}

impl Phone {
    async fn live_until(&self, what: &str, f: impl Fn(&Value) -> bool) -> Value {
        for _ in 0..100 {
            let (_, body) = self.get("/api/live", Some(&self.token)).await;
            if f(&body["data"]) {
                return body["data"].clone();
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("timed out waiting for {what}");
    }
}

impl Phone {
    async fn send(
        &self,
        method: reqwest::Method,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut req = self.http.request(method, format!("{}{path}", self.base));
        if let Some(token) = token {
            req = req.bearer_auth(token);
        }
        if let Some(body) = body {
            req = req.json(&body);
        }
        let res = req.send().await.unwrap();
        let status = StatusCode::from_u16(res.status().as_u16()).unwrap();
        (status, res.json().await.unwrap_or(Value::Null))
    }

    async fn get(&self, path: &str, token: Option<&str>) -> (StatusCode, Value) {
        self.send(reqwest::Method::GET, path, token, None).await
    }
}

#[tokio::test]
async fn every_api_call_needs_the_token() {
    let p = phone().await;
    let (status, body) = p.get("/api/live", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"]["code"], "unauthorized");
    assert_eq!(
        p.get("/api/live", Some("wrong")).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, body) = p.get("/api/live", Some(&p.token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], true);
    assert!(body["data"]["version"].as_u64().unwrap() >= 1000);
    let (_, packs) = p.get("/api/packs", Some(&p.token)).await;
    assert_eq!(packs["data"].as_array().unwrap().len(), 11);
    p.app.shutdown().await;
}

#[tokio::test]
async fn repeated_failures_lock_the_address_out() {
    let p = phone().await;
    for _ in 0..10 {
        p.get("/api/live", Some("wrong")).await;
    }
    let (status, body) = p.get("/api/live", Some(&p.token)).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["error"]["code"], "rate_limited");
    p.app.shutdown().await;
}

#[tokio::test]
async fn edits_are_validated_with_clear_errors() {
    let p = phone().await;
    let t = Some(p.token.as_str());
    let post = |body: Value| p.send(reqwest::Method::POST, "/api/mmr", t, Some(body));
    assert_eq!(
        post(json!({"team_size": 4, "mmr": 900})).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        post(json!({"team_size": 2, "mmr": 3001})).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        post(json!({"team_size": 2, "mmr": 900.5})).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        post(json!({"team_size": 2, "mmr": 900, "extra": 1}))
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (status, body) = post(json!({"team_size": 2, "mmr": 900})).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["data"]["id"].as_i64().is_some());
    let put = |path: &'static str, body: Value| p.send(reqwest::Method::PUT, path, t, Some(body));
    let (status, body) = put("/api/blocks/999/pack", json!({"pack_id": 1})).await;
    assert_eq!(
        (status, body["error"]["code"].as_str()),
        (StatusCode::NOT_FOUND, Some("not_found"))
    );
    assert_eq!(
        put("/api/blocks/abc/pack", json!({"pack_id": 1})).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        put("/api/attempts/999/skipped", json!({"skipped": true}))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        put("/api/sessions/999/notes", json!({"notes": "x"}))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (status, body) = p.get("/api/nope", t).await;
    assert_eq!(
        (status, body["error"]["code"].as_str()),
        (StatusCode::NOT_FOUND, Some("not_found"))
    );
    p.app.shutdown().await;
}

#[tokio::test]
async fn regenerating_the_token_revokes_the_old_one() {
    let p = phone().await;
    let pairing = p.app.regenerate_token().await.unwrap().unwrap();
    let fresh = pairing.url.split("#t=").nth(1).unwrap().to_owned();
    assert_ne!(fresh, p.token);
    assert_eq!(
        p.get("/api/live", Some(&p.token)).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(p.get("/api/live", Some(&fresh)).await.0, StatusCode::OK);
    p.app.shutdown().await;
}

async fn oneshot(router: axum::Router, req: Request<Body>) -> (StatusCode, Value) {
    let res = router.oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn host_and_body_size_are_guarded() {
    let p = phone().await;
    let router = || {
        phone::router(p.app.clone(), p.app.token_receiver())
            .layer(MockConnectInfo(SocketAddr::from(([127, 0, 0, 1], 1))))
    };
    let req = |host: &str| {
        Request::get("/api/live")
            .header("host", host)
            .header("authorization", format!("Bearer {}", p.token))
            .body(Body::empty())
            .unwrap()
    };
    let (status, body) = oneshot(router(), req("evil.example")).await;
    assert_eq!(
        (status, body["error"]["code"].as_str()),
        (StatusCode::FORBIDDEN, Some("forbidden"))
    );
    assert_eq!(
        oneshot(router(), req("192.168.1.20:49321")).await.0,
        StatusCode::OK
    );
    let big = json!({"notes": "x".repeat(20 * 1024)}).to_string();
    let req = Request::put("/api/sessions/1/notes")
        .header("host", "127.0.0.1")
        .header("authorization", format!("Bearer {}", p.token))
        .header("content-type", "application/json")
        .body(Body::from(big))
        .unwrap();
    assert_eq!(
        oneshot(router(), req).await.0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    p.app.shutdown().await;
}

#[tokio::test]
async fn a_finished_session_is_reviewed_exactly_once() {
    let p =
        phone_seeded("INSERT INTO session (id, started_at, ended_at) VALUES (7, 0, 1000);").await;
    let t = Some(p.token.as_str());
    let review = |body: Value| {
        p.send(
            reqwest::Method::POST,
            "/api/sessions/7/review",
            t,
            Some(body),
        )
    };
    let (status, _) = review(json!({"mmr": [{"team_size": 2, "mmr": 3001}], "notes": "x"})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = review(json!({"mmr": [], "extra": 1})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, body) =
        review(json!({"mmr": [{"team_size": 2, "mmr": 912}], "notes": "good"})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (
            body["data"]["reviewed"].as_bool(),
            body["data"]["notes"].as_str()
        ),
        (Some(true), Some("good"))
    );
    let (status, body) = review(json!({"mmr": [], "notes": null})).await;
    assert_eq!(
        (status, body["error"]["code"].as_str()),
        (StatusCode::CONFLICT, Some("conflict"))
    );
    let live = p
        .live_until("the review", |d| {
            d["live"]["last_session"]["reviewed"] == true
        })
        .await;
    assert_eq!(
        live["live"]["latest_mmr"].as_array().unwrap().len(),
        1,
        "the invalid try wrote nothing"
    );
    assert_eq!(live["live"]["latest_mmr"][0]["mmr"], 912);
    let (status, _) = p
        .send(
            reqwest::Method::POST,
            "/api/sessions/99/review",
            t,
            Some(json!({"mmr": []})),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    p.app.shutdown().await;
}

#[tokio::test]
async fn packs_come_with_their_statistics() {
    let p = phone().await;
    let (_, body) = p.get("/api/packs", Some(&p.token)).await;
    let first = &body["data"][0];
    assert_eq!(first["pack"]["name"], "Aerial Off Wall");
    assert_eq!(
        (first["attempts"].as_i64(), first["make_rate"].is_null()),
        (Some(0), true)
    );
    p.app.shutdown().await;
}

#[tokio::test]
async fn the_session_review_can_be_read_with_the_token() {
    let p =
        phone_seeded("INSERT INTO session (id, started_at, ended_at) VALUES (7, 0, 1000);").await;
    let (status, body) = p.get("/api/sessions/7/review", Some(&p.token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ok"], true);
    assert_eq!(body["data"]["session_id"], 7);
    let (status, body) = p.get("/api/sessions/99/review", Some(&p.token)).await;
    assert_eq!((status, body["data"].is_null()), (StatusCode::OK, true));
    let (status, _) = p.get("/api/sessions/7/review", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    p.app.shutdown().await;
}

#[tokio::test]
async fn a_bad_packs_query_answers_the_json_error_envelope() {
    let p = phone().await;
    // `search` twice cannot fill the one `search` field.
    let (status, body) = p.get("/api/packs?search=a&search=b", Some(&p.token)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"]["code"], "invalid");
    p.app.shutdown().await;
}
