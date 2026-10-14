//! The phone page, embedded in the binary from `ui/phone/`.

use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::Embed;

use crate::phone::api::ApiError;

#[derive(Embed)]
#[folder = "../../ui/dist/phone/"]
struct PhoneUi;

pub(crate) fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "json" => "application/json",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        _ => "application/octet-stream",
    }
}

/// The page may not be framed (clickjacking) and loads nothing from other origins.
const CSP: &str = concat!(
    "default-src 'self'; connect-src 'self' ws: wss:; img-src 'self' data:; ",
    "style-src 'self' 'unsafe-inline'; frame-ancestors 'none'"
);

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path == "api" || path.starts_with("api/") {
        return ApiError::not_found("no such API route").into_response();
    }
    let path = if path.is_empty() { "index.html" } else { path };
    match PhoneUi::get(path) {
        Some(file) => (
            [
                (header::CONTENT_TYPE, content_type(path)),
                (header::CACHE_CONTROL, "no-cache"),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
                (header::REFERRER_POLICY, "no-referrer"),
                (header::X_FRAME_OPTIONS, "DENY"),
                (header::CONTENT_SECURITY_POLICY, CSP),
            ],
            file.data.into_owned(),
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::content_type;

    #[test]
    fn the_keep_awake_video_is_served_as_video() {
        // `nosniff` is set, so the browser must be told the real type to play it.
        assert_eq!(content_type("assets/blank-1a2b.mp4"), "video/mp4");
        assert_eq!(content_type("assets/blank-3c4d.webm"), "video/webm");
    }

    #[test]
    fn the_csp_is_one_clean_line() {
        assert!(!super::CSP.contains("  "), "{}", super::CSP);
        assert!(super::CSP.contains("; style-src 'self' 'unsafe-inline';"));
    }

    #[test]
    fn fonts_are_served_as_fonts() {
        assert_eq!(content_type("assets/manrope-latin-400.woff2"), "font/woff2");
        assert_eq!(content_type("assets/manrope-latin-400.woff"), "font/woff");
    }
}
