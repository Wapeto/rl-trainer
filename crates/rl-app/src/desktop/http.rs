//! Serves the desktop page and its commands on 127.0.0.1 for development and end-to-end tests.
//! Never started by rl-trainer.exe (the window talks to Tauri there).

use std::net::SocketAddr;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use rust_embed::Embed;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use crate::app::App;
use crate::phone::assets::content_type;

#[derive(Embed)]
#[folder = "../../ui/dist/desktop/"]
struct DesktopUi;

const CSP: &str = concat!(
    "default-src 'self'; connect-src 'self'; img-src 'self' data:; ",
    "style-src 'self' 'unsafe-inline'; frame-ancestors 'none'"
);

pub fn router(app: App) -> Router {
    Router::new()
        .route("/desktop-api/{command}", post(call))
        .fallback(page)
        .with_state(app)
}

/// Binds `127.0.0.1:port` (0 picks one) and serves until the task is dropped or the app ends.
pub async fn start(app: App, port: u16) -> std::io::Result<(SocketAddr, JoinHandle<()>)> {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port))).await?;
    let addr = listener.local_addr()?;
    let join = tokio::spawn(async move {
        if let Err(err) = axum::serve(listener, router(app)).await {
            tracing::error!(error = %err, "desktop page server stopped");
        }
    });
    Ok((addr, join))
}

/// Only pages on this PC may call: a custom header forces a CORS preflight, which this server
/// never grants, and the Host check stops DNS rebinding.
fn allowed(headers: &HeaderMap) -> bool {
    let local = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| {
            let name = h.rsplit_once(':').map_or(h, |(name, _)| name);
            name == "127.0.0.1" || name == "localhost"
        });
    let marked = headers
        .get("x-rl-desktop")
        .is_some_and(|v| v.as_bytes() == b"1");
    local && marked
}

fn failure(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(json!({"ok": false, "error": {"message": message}})),
    )
        .into_response()
}

async fn call(
    State(app): State<App>,
    Path(command): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !allowed(&headers) {
        return failure(StatusCode::FORBIDDEN, "forbidden");
    }
    let args = if body.is_empty() {
        Value::Null
    } else {
        match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(e) => return failure(StatusCode::BAD_REQUEST, &format!("bad arguments: {e}")),
        }
    };
    match super::dispatch(&app, &command, args).await {
        Ok(data) => Json(json!({"ok": true, "data": data})).into_response(),
        Err(message) => failure(StatusCode::BAD_REQUEST, &message),
    }
}

async fn page(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    match DesktopUi::get(path) {
        Some(file) => (
            [
                (header::CONTENT_TYPE, content_type(path)),
                (header::CACHE_CONTROL, "no-cache"),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
                (header::CONTENT_SECURITY_POLICY, CSP),
            ],
            file.data.into_owned(),
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}
