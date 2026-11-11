use axum::{
    body::Body,
    http::{header, HeaderValue},
    response::Response,
};

/// Error bodies larger than this are not echoed back (they are never meant for the user).
const MAX_DETAIL_BYTES: usize = 16 * 1024;

/// The frontend reads error messages from a JSON `{"detail": ...}` body, but most handlers
/// (and axum's own extractor rejections) answer errors as plain text, which hid every message
/// behind "Permintaan gagal (4xx)". This rewrites non-JSON error responses into that shape.
pub async fn wrap_plain_errors(response: Response) -> Response {
    let status = response.status();
    if !(status.is_client_error() || status.is_server_error()) {
        return response;
    }
    let is_json = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.starts_with("application/json"));
    if is_json {
        return response;
    }

    let (mut parts, body) = response.into_parts();
    let text = axum::body::to_bytes(body, MAX_DETAIL_BYTES)
        .await
        .map(|b| String::from_utf8_lossy(&b).trim().to_string())
        .unwrap_or_default();
    let detail = if text.is_empty() {
        status.canonical_reason().unwrap_or("Error").to_string()
    } else {
        text
    };

    let json = serde_json::json!({ "detail": detail }).to_string();
    parts.headers.remove(header::CONTENT_LENGTH);
    parts
        .headers
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
    Response::from_parts(parts, Body::from(json))
}
