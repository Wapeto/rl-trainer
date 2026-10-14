mod common;

use std::net::SocketAddr;
use std::time::Duration;

use common::*;
use futures_util::{SinkExt, StreamExt};
use rl_app::devtools::{event_json, goal_json, update_state_json, FakeStatsApi};
use rl_app::testing::FakePlatform;
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

fn training_script() -> Vec<(Duration, String)> {
    let step = Duration::from_millis(400);
    let t = |time: f64| update_state_json(21, time);
    vec![
        (Duration::ZERO, t(10.0)),
        (step, event_json("RoundStarted")),
        (step, t(9.0)),
        (step, t(8.0)),
        (step, goal_json("Me")),
        (step, event_json("RoundStarted")), // attempt 1 (make) recorded
        (step, t(10.0)),
        (step, t(9.0)),
        (step, t(8.0)),
        (step, event_json("RoundStarted")), // attempt 2 (miss) recorded
    ]
}

async fn connect(port: u16, token: &str) -> Socket {
    let (mut ws, _) =
        tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/api/live/ws"))
            .await
            .unwrap();
    ws.send(Message::Text(
        json!({"type": "auth", "token": token}).to_string().into(),
    ))
    .await
    .unwrap();
    ws
}

async fn next_snapshot(ws: &mut Socket, what: &str, mut f: impl FnMut(&Value) -> bool) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let msg = tokio::time::timeout_at(deadline, ws.next())
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
            .expect("socket open")
            .unwrap();
        if let Message::Text(text) = msg {
            let value: Value = serde_json::from_str(text.as_str()).unwrap();
            if value["type"] == "snapshot" && f(&value["data"]) {
                return value["data"].clone();
            }
        }
    }
}

async fn close_code(ws: &mut Socket) -> u16 {
    loop {
        match tokio::time::timeout(Duration::from_secs(10), ws.next())
            .await
            .expect("closed in time")
        {
            Some(Ok(Message::Close(Some(frame)))) => return u16::from(frame.code),
            Some(Ok(_)) => continue,
            other => panic!("expected a close frame, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn phone_drives_a_training_session_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let api = FakeStatsApi::start(training_script(), None).await.unwrap();
    let exe = install(dir.path(), &ini_text(10, port_of(&api.addr)));
    let platform = FakePlatform::new();
    platform.set_game(game(&exe));
    let app = start(&dir, platform, None).await;
    let pairing = app
        .enable_phone_at(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let token = pairing.url.split("#t=").nth(1).unwrap().to_owned();
    let base = format!("http://127.0.0.1:{}", pairing.port);
    let http = reqwest::Client::new();
    let mut ws = connect(pairing.port, &token).await;

    let s = next_snapshot(&mut ws, "two attempts", |s| {
        s["live"]["block"]["training"]["recent"]
            .as_array()
            .is_some_and(|r| r.len() == 2)
    })
    .await;
    assert_eq!(s["phone_clients"], 1);
    let block = s["live"]["block"]["id"].as_i64().unwrap();
    let last = s["live"]["block"]["training"]["recent"][1]["id"]
        .as_i64()
        .unwrap();
    let session = s["live"]["session"]["id"].as_i64().unwrap();

    let put = |path: String, body: Value| {
        http.put(format!("{base}{path}"))
            .bearer_auth(&token)
            .json(&body)
            .send()
    };
    assert!(
        put(format!("/api/blocks/{block}/pack"), json!({"pack_id": 6}))
            .await
            .unwrap()
            .status()
            .is_success()
    );
    let s = next_snapshot(&mut ws, "the split block with the pack", |s| {
        s["live"]["block"]["pack"]["id"] == 6
    })
    .await;
    assert_ne!(
        s["live"]["block"]["id"].as_i64().unwrap(),
        block,
        "attempts existed: the block split"
    );

    assert!(put(
        format!("/api/attempts/{last}/skipped"),
        json!({"skipped": true})
    )
    .await
    .unwrap()
    .status()
    .is_success());
    let res = http
        .post(format!("{base}/api/mmr"))
        .bearer_auth(&token)
        .json(&json!({"team_size": 2, "mmr": 899, "session_id": session}))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
    next_snapshot(&mut ws, "the MMR", |s| {
        s["live"]["latest_mmr"][0]["mmr"] == 899
    })
    .await;
    assert!(put(
        format!("/api/sessions/{session}/notes"),
        json!({"notes": "good aerials"})
    )
    .await
    .unwrap()
    .status()
    .is_success());
    next_snapshot(&mut ws, "the notes", |s| {
        s["live"]["session"]["notes"] == "good aerials"
    })
    .await;

    // Restoring while the phone is connected: the socket stays up and sees the version jump.
    let version = s["version"].as_u64().unwrap();
    let daily = app
        .list_backups()
        .unwrap()
        .into_iter()
        .find(|b| b.name.starts_with("rl-trainer-"))
        .unwrap();
    app.restore_backup(&daily.name).await.unwrap();
    next_snapshot(&mut ws, "the restored state", |s| {
        s["version"].as_u64().unwrap() >= version + 1000
    })
    .await;
    app.shutdown().await;
}

#[tokio::test]
async fn sockets_without_a_valid_token_are_closed_with_4401() {
    let dir = tempfile::tempdir().unwrap();
    let app = start(&dir, FakePlatform::new(), None).await;
    let pairing = app
        .enable_phone_at(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let mut wrong = connect(pairing.port, "wrong").await;
    assert_eq!(close_code(&mut wrong).await, 4401);

    let (mut silent, _) =
        tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}/api/live/ws", pairing.port))
            .await
            .unwrap();
    assert_eq!(
        close_code(&mut silent).await,
        4401,
        "no auth message within 5 s"
    );

    let token = pairing.url.split("#t=").nth(1).unwrap().to_owned();
    let mut paired = connect(pairing.port, &token).await;
    next_snapshot(&mut paired, "the first snapshot", |_| true).await;
    app.regenerate_token().await.unwrap();
    assert_eq!(
        close_code(&mut paired).await,
        4401,
        "a new token revokes open sockets"
    );
    app.shutdown().await;
}

#[tokio::test]
async fn the_phone_page_is_served_without_a_token() {
    let dir = tempfile::tempdir().unwrap();
    let app = start(&dir, FakePlatform::new(), None).await;
    let pairing = app
        .enable_phone_at(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let res = reqwest::get(format!("http://127.0.0.1:{}/", pairing.port))
        .await
        .unwrap();
    assert!(res.status().is_success());
    assert_eq!(res.headers()["x-frame-options"], "DENY");
    let csp = res.headers()["content-security-policy"]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        csp.contains("frame-ancestors 'none'") && csp.contains("default-src 'self'"),
        "{csp}"
    );
    assert!(res.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("text/html"));
    let html = res.text().await.unwrap();
    assert!(
        html.contains("<div id=\"root\"></div>"),
        "the built React page is served"
    );
    assert!(html.contains("/assets/"), "with its bundled assets");
    app.shutdown().await;
}

#[tokio::test]
async fn an_oversized_message_before_auth_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let app = start(&dir, FakePlatform::new(), None).await;
    let pairing = app
        .enable_phone_at(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let token = pairing.url.split("#t=").nth(1).unwrap().to_owned();
    let (mut ws, _) =
        tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}/api/live/ws", pairing.port))
            .await
            .unwrap();
    // A valid auth message padded to 8 KiB: it must be refused for its size, not parsed.
    let padded = json!({"type": "auth", "token": token, "pad": "x".repeat(8 * 1024)});
    let _ = ws.send(Message::Text(padded.to_string().into())).await;
    let got_snapshot = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(msg) = ws.next().await {
            match msg {
                Ok(Message::Text(text)) if text.as_str().contains("\"snapshot\"") => return true,
                Ok(Message::Close(_)) | Err(_) => return false,
                Ok(_) => {}
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    assert!(!got_snapshot);
    app.shutdown().await;
}

#[tokio::test]
async fn a_token_regenerated_during_auth_never_leaves_a_socket_open() {
    let dir = tempfile::tempdir().unwrap();
    let app = start(&dir, FakePlatform::new(), None).await;
    let pairing = app
        .enable_phone_at(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    for _ in 0..10 {
        let current = app.pairing().await.unwrap();
        let token = current.url.split("#t=").nth(1).unwrap().to_owned();
        let mut ws = connect(pairing.port, &token).await;
        app.regenerate_token().await.unwrap();
        assert_eq!(close_code(&mut ws).await, 4401);
    }
    app.shutdown().await;
}
