use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::Router;
use tower::ServiceExt;
use xterm_backend::controllers::request_guard::{guard, GuardConfig};

fn app(loopback_only: bool) -> Router {
    Router::new()
        .route("/api/servers", get(|| async { "servers" }))
        .route("/api/auth/status", get(|| async { "status" }))
        .route("/api/files/1/download", get(|| async { "file" }))
        .route("/ws/terminal", get(|| async { "ws" }))
        .route("/", get(|| async { "index" }))
        .layer(axum::middleware::from_fn_with_state(
            GuardConfig { token: "s3cret".into(), loopback_only },
            guard,
        ))
}

async fn call(app: Router, uri: &str, host: &str, token: Option<&str>) -> (StatusCode, String) {
    let mut req = Request::builder().uri(uri).header("Host", host);
    if let Some(t) = token {
        req = req.header("X-Token", t);
    }
    let res = app.oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
    let status = res.status();
    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

#[tokio::test]
async fn api_requires_the_token() {
    let host = "127.0.0.1:1945";
    assert_eq!(call(app(true), "/api/servers", host, Some("s3cret")).await.0, StatusCode::OK);
    let (status, body) = call(app(true), "/api/servers", host, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(body.contains("\"detail\""), "{}", body);
    assert_eq!(call(app(true), "/api/servers", host, Some("salah")).await.0, StatusCode::FORBIDDEN);
    // Links opened by the browser carry the token in the query string.
    assert_eq!(call(app(true), "/api/files/1/download?path=/x&token=s3cret", host, None).await.0, StatusCode::OK);
    assert_eq!(call(app(true), "/api/files/1/download?path=/x&token=bad", host, None).await.0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn status_static_and_ws_do_not_need_the_token() {
    let host = "localhost:1945";
    assert_eq!(call(app(true), "/api/auth/status", host, None).await.0, StatusCode::OK);
    assert_eq!(call(app(true), "/", host, None).await.0, StatusCode::OK);
    assert_eq!(call(app(true), "/ws/terminal", host, None).await.0, StatusCode::OK);
}

#[tokio::test]
async fn foreign_host_is_rejected_in_loopback_mode() {
    // DNS rebinding: an attacker's domain resolving to 127.0.0.1.
    assert_eq!(call(app(true), "/", "evil.example:1945", None).await.0, StatusCode::FORBIDDEN);
    assert_eq!(call(app(true), "/api/auth/status", "evil.example", None).await.0, StatusCode::FORBIDDEN);
    assert_eq!(call(app(true), "/", "[::1]:1945", None).await.0, StatusCode::OK);
    // LAN mode (--host 0.0.0.0) is reached through the machine's own IP or name.
    assert_eq!(call(app(false), "/", "192.168.1.20:1945", None).await.0, StatusCode::OK);
    assert_eq!(call(app(false), "/api/servers", "192.168.1.20:1945", None).await.0, StatusCode::FORBIDDEN);
}
