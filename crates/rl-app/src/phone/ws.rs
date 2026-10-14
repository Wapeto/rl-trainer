//! Live snapshots over WebSocket. Browsers cannot set headers on a WebSocket, so the token is
//! the first message instead of a URL parameter (which would end up in logs and history).

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::response::Response;
use serde::Deserialize;
use serde_json::json;

use crate::phone::guard::token_matches;
use crate::phone::PhoneState;

pub const AUTH_TIMEOUT: Duration = Duration::from_secs(5);
pub const UNAUTHORIZED: u16 = 4401;
/// Clients only ever send the auth message; anything larger is refused before it is buffered.
pub const MAX_CLIENT_MESSAGE_BYTES: usize = 4 * 1024;
/// A client that stops reading must not block the revoke of a regenerated token.
pub const SEND_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Deserialize)]
struct AuthMessage {
    #[serde(rename = "type")]
    kind: String,
    token: String,
}

pub fn auth_message_ok(text: &str, expected: &str) -> bool {
    serde_json::from_str::<AuthMessage>(text)
        .is_ok_and(|m| m.kind == "auth" && token_matches(expected, &m.token))
}

pub async fn upgrade(
    State(s): State<PhoneState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.max_message_size(MAX_CLIENT_MESSAGE_BYTES)
        .max_frame_size(MAX_CLIENT_MESSAGE_BYTES)
        .on_upgrade(move |socket| client(socket, s, peer))
}

async fn close(mut socket: WebSocket) {
    let frame = CloseFrame {
        code: UNAUTHORIZED,
        reason: "unauthorized".into(),
    };
    let _ = tokio::time::timeout(SEND_TIMEOUT, socket.send(Message::Close(Some(frame)))).await;
}

fn snapshot_message(s: &PhoneState) -> Message {
    let body = json!({"type": "snapshot", "data": s.app.snapshot()});
    Message::Text(body.to_string().into())
}

async fn send(socket: &mut WebSocket, message: Message) -> bool {
    matches!(
        tokio::time::timeout(SEND_TIMEOUT, socket.send(message)).await,
        Ok(Ok(()))
    )
}

async fn client(mut socket: WebSocket, s: PhoneState, peer: SocketAddr) {
    if s.auth.is_locked(peer.ip(), Instant::now()) {
        return close(socket).await;
    }
    // Take the token watch before checking: a regeneration during the auth exchange is then
    // seen as a change below and closes the socket.
    let mut token = s.token.clone();
    let first = tokio::time::timeout(AUTH_TIMEOUT, socket.recv()).await;
    let expected = token.borrow_and_update().clone();
    let authed = match &first {
        Ok(Some(Ok(Message::Text(text)))) => auth_message_ok(text.as_str(), &expected),
        _ => false,
    };
    if !authed {
        s.auth.record_failure(peer.ip(), Instant::now());
        return close(socket).await;
    }
    let mut snapshots = s.app.snapshots();
    s.app.phone_client_changed(true).await;
    if send(&mut socket, snapshot_message(&s)).await {
        loop {
            tokio::select! {
                changed = snapshots.changed() => {
                    if changed.is_err() || !send(&mut socket, snapshot_message(&s)).await {
                        break;
                    }
                }
                _ = token.changed() => {
                    close(socket).await;
                    s.app.phone_client_changed(false).await;
                    return;
                }
                incoming = socket.recv() => match incoming {
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                    Some(Ok(_)) => {} // pings are answered by axum; other input is ignored
                },
            }
        }
    }
    s.app.phone_client_changed(false).await;
}
