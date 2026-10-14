//! The LAN phone server: static UI, token-protected REST API and the live WebSocket.

pub mod api;
pub mod assets;
pub mod guard;
pub mod ws;

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::DefaultBodyLimit;
use axum::routing::{any, get, post, put};
use axum::{middleware, Router};
use tokio::net::TcpListener;
use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;

use crate::app::App;
use guard::{AuthLimiter, RateLimiter, PER_ADDRESS_PER_SECOND};

pub const MAX_BODY_BYTES: usize = 16 * 1024;

#[derive(Clone)]
pub struct PhoneState {
    pub app: App,
    pub token: watch::Receiver<String>,
    pub auth: Arc<AuthLimiter>,
    pub rate: Arc<RateLimiter>,
}

pub fn router(app: App, token: watch::Receiver<String>) -> Router {
    let state = PhoneState {
        app,
        token,
        auth: Arc::new(AuthLimiter::default()),
        rate: Arc::new(RateLimiter::new(
            PER_ADDRESS_PER_SECOND,
            PER_ADDRESS_PER_SECOND,
        )),
    };
    let protected = Router::new()
        .route("/api/live", get(api::live))
        .route("/api/packs", get(api::packs))
        .route("/api/blocks/{id}/pack", put(api::set_pack))
        .route("/api/attempts/{id}/skipped", put(api::set_skipped))
        .route("/api/mmr", post(api::add_mmr))
        .route("/api/sessions/{id}/notes", put(api::set_notes))
        .route(
            "/api/sessions/{id}/review",
            post(api::review).get(api::session_review),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            guard::require_token,
        ));
    Router::new()
        .merge(protected)
        .route("/api/live/ws", get(ws::upgrade))
        .route("/api/{*rest}", any(api::api_not_found))
        .fallback(assets::serve)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(middleware::from_fn_with_state(state.clone(), guard::guard))
        .with_state(state)
}

pub struct PhoneServer {
    pub local_addr: SocketAddr,
    stop: oneshot::Sender<()>,
    join: JoinHandle<()>,
}

pub async fn start(
    app: App,
    bind: SocketAddr,
    token: watch::Receiver<String>,
) -> io::Result<PhoneServer> {
    let listener = TcpListener::bind(bind).await?;
    let local_addr = listener.local_addr()?;
    let service = router(app, token).into_make_service_with_connect_info::<SocketAddr>();
    let (stop, stopped) = oneshot::channel::<()>();
    let join = tokio::spawn(async move {
        let served = axum::serve(listener, service)
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await;
        if let Err(err) = served {
            tracing::error!(error = %err, "phone server stopped");
        }
    });
    tracing::info!(%local_addr, "phone server listening");
    Ok(PhoneServer {
        local_addr,
        stop,
        join,
    })
}

impl PhoneServer {
    /// Graceful stop; open WebSockets that do not close within 2 s are cut.
    pub async fn stop(self) {
        let PhoneServer { stop, mut join, .. } = self;
        let _ = stop.send(());
        if tokio::time::timeout(Duration::from_secs(2), &mut join)
            .await
            .is_err()
        {
            join.abort();
        }
    }
}
