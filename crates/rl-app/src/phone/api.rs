//! JSON envelope, error mapping and REST handlers.

use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{FromRequest, FromRequestParts, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use rl_core::store::{EditError, MmrInput};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::app::AppError;
use crate::engine::CommandError;
use crate::phone::PhoneState;

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
    pub fn unauthorized() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "pair this device again from the PC",
        )
    }
    pub fn forbidden(message: &str) -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", message)
    }
    pub fn rate_limited() -> Self {
        Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "too many requests, wait a moment",
        )
    }
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", message)
    }
    fn internal() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "something went wrong on the PC; see its log",
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = json!({"ok": false, "error": {"code": self.code, "message": self.message}});
        (self.status, Json(body)).into_response()
    }
}

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        Self::new(rejection.status(), "invalid", rejection.body_text())
    }
}

impl From<PathRejection> for ApiError {
    fn from(rejection: PathRejection) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid", rejection.body_text())
    }
}

impl From<EditError> for ApiError {
    fn from(err: EditError) -> Self {
        match err {
            EditError::NotFound(what) => Self::not_found(format!("{what} not found")),
            EditError::Conflict(message) => Self::new(StatusCode::CONFLICT, "conflict", message),
            EditError::Invalid(message) => {
                Self::new(StatusCode::UNPROCESSABLE_ENTITY, "invalid", message)
            }
            EditError::Store(err) => {
                tracing::error!(error = %err, "phone edit failed");
                Self::internal()
            }
        }
    }
}

impl From<AppError> for ApiError {
    fn from(err: AppError) -> Self {
        match err {
            AppError::Command(CommandError::Edit(edit)) => edit.into(),
            AppError::Command(CommandError::Stopped) => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "the recorder is restarting, try again",
            ),
            other => {
                tracing::error!(error = %other, "phone request failed");
                Self::internal()
            }
        }
    }
}

pub fn ok<T: Serialize>(data: T) -> Json<Value> {
    Json(json!({"ok": true, "data": data}))
}

#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(ApiError))]
pub struct ApiJson<T>(pub T);

#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Path), rejection(ApiError))]
pub struct ApiPath<T>(pub T);

/// A query string that answers the JSON error envelope when it cannot be read.
#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(ApiError))]
pub struct ApiQuery<T>(pub T);

impl From<axum::extract::rejection::QueryRejection> for ApiError {
    fn from(rejection: axum::extract::rejection::QueryRejection) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid", rejection.body_text())
    }
}

pub async fn live(State(s): State<PhoneState>) -> Json<Value> {
    ok(s.app.snapshot())
}

#[derive(Deserialize, Default)]
pub struct PackQuery {
    #[serde(default)]
    search: String,
}

pub async fn packs(
    State(s): State<PhoneState>,
    ApiQuery(q): ApiQuery<PackQuery>,
) -> Result<Json<Value>, ApiError> {
    Ok(ok(s.app.packs(&q.search)?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackBody {
    pub pack_id: Option<i64>,
}

pub async fn set_pack(
    State(s): State<PhoneState>,
    ApiPath(block_id): ApiPath<i64>,
    ApiJson(body): ApiJson<PackBody>,
) -> Result<Json<Value>, ApiError> {
    s.app.set_block_pack(block_id, body.pack_id).await?;
    Ok(ok(json!({"block_id": block_id, "pack_id": body.pack_id})))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkipBody {
    pub skipped: bool,
}

pub async fn set_skipped(
    State(s): State<PhoneState>,
    ApiPath(attempt_id): ApiPath<i64>,
    ApiJson(body): ApiJson<SkipBody>,
) -> Result<Json<Value>, ApiError> {
    s.app.set_attempt_skipped(attempt_id, body.skipped).await?;
    Ok(ok(
        json!({"attempt_id": attempt_id, "skipped": body.skipped}),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MmrBody {
    pub team_size: u8,
    pub mmr: i32,
    #[serde(default)]
    pub session_id: Option<i64>,
}

pub async fn add_mmr(
    State(s): State<PhoneState>,
    ApiJson(body): ApiJson<MmrBody>,
) -> Result<Json<Value>, ApiError> {
    let id = s
        .app
        .add_mmr(body.team_size, body.mmr, body.session_id)
        .await?;
    Ok(ok(json!({"id": id})))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotesBody {
    pub notes: String,
}

pub async fn set_notes(
    State(s): State<PhoneState>,
    ApiPath(session_id): ApiPath<i64>,
    ApiJson(body): ApiJson<NotesBody>,
) -> Result<Json<Value>, ApiError> {
    let notes = s.app.set_session_notes(session_id, body.notes).await?;
    Ok(ok(json!({"notes": notes})))
}

pub async fn api_not_found() -> ApiError {
    ApiError::not_found("no such API route")
}

#[derive(Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ReviewBody {
    pub mmr: Vec<MmrInput>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// The after-session review (spec §5.2), `null` for an unknown session.
pub async fn session_review(
    State(s): State<PhoneState>,
    ApiPath(session_id): ApiPath<i64>,
) -> Result<Json<Value>, ApiError> {
    Ok(ok(s.app.session_review(session_id)?))
}

pub async fn review(
    State(s): State<PhoneState>,
    ApiPath(session_id): ApiPath<i64>,
    ApiJson(body): ApiJson<ReviewBody>,
) -> Result<Json<Value>, ApiError> {
    let summary = s
        .app
        .review_session(session_id, body.mmr, body.notes)
        .await?;
    Ok(ok(summary))
}
