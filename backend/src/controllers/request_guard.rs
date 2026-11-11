use axum::{
    body::Body,
    extract::{Query, Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use std::collections::HashMap;

#[derive(Clone)]
pub struct GuardConfig {
    /// Per-process token embedded in index.html; the frontend sends it as `X-Token`
    /// (or `?token=` on links the browser opens itself).
    pub token: String,
    /// Bound to loopback: requests must name a loopback `Host` (defeats DNS rebinding).
    pub loopback_only: bool,
}

fn forbidden(detail: &str) -> Response {
    Response::builder()
        .status(StatusCode::FORBIDDEN)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::json!({ "detail": detail }).to_string()))
        .unwrap()
}

/// `Host` header without its port; IPv6 literals keep their brackets.
fn host_name(host: &str) -> &str {
    if host.starts_with('[') {
        return host.find(']').map(|i| &host[..=i]).unwrap_or(host);
    }
    host.rsplit_once(':').map(|(h, _)| h).unwrap_or(host)
}

fn is_loopback_host(host: &str) -> bool {
    matches!(host_name(host), "127.0.0.1" | "localhost" | "[::1]")
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Rejects requests from foreign origins before they reach any handler.
pub async fn guard(State(cfg): State<GuardConfig>, req: Request, next: Next) -> Response {
    if cfg.loopback_only {
        let host = req
            .headers()
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !is_loopback_host(host) {
            return forbidden("Host tidak diizinkan");
        }
    }

    let path = req.uri().path();
    if path.starts_with("/api/") && path != "/api/auth/status" {
        let header_token = req
            .headers()
            .get("X-Token")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let token = header_token.or_else(|| {
            Query::<HashMap<String, String>>::try_from_uri(req.uri())
                .ok()
                .and_then(|q| q.0.get("token").cloned())
        });
        let valid = token
            .map(|t| constant_time_eq(t.as_bytes(), cfg.token.as_bytes()))
            .unwrap_or(false);
        if !valid {
            return forbidden("Token tidak valid. Muat ulang halaman RusTerm.");
        }
    }

    next.run(req).await
}
