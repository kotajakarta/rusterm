use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use tower::ServiceExt;
use xterm_backend::controllers::api_errors::wrap_plain_errors;

fn app() -> Router {
    Router::new()
        .route("/plain", get(|| async { (StatusCode::BAD_REQUEST, "Invalid PIN".to_string()) }))
        .route("/bare", get(|| async { StatusCode::UNAUTHORIZED }))
        .route(
            "/json",
            get(|| async {
                (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "detail": "Server tidak ditemukan" })),
                )
            }),
        )
        .route("/ok", get(|| async { "hello" }))
        .layer(axum::middleware::map_response(wrap_plain_errors))
}

async fn call(uri: &str) -> (StatusCode, String, String) {
    let res = app()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let ct = res
        .headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default();
    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    (status, ct, String::from_utf8(body.to_vec()).unwrap())
}

#[tokio::test]
async fn plain_text_errors_become_json_detail() {
    let (status, ct, body) = call("/plain").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(ct.starts_with("application/json"), "{}", ct);
    assert_eq!(body, r#"{"detail":"Invalid PIN"}"#);
}

#[tokio::test]
async fn empty_errors_get_reason_phrase() {
    let (status, ct, body) = call("/bare").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(ct.starts_with("application/json"));
    assert_eq!(body, r#"{"detail":"Unauthorized"}"#);
}

#[tokio::test]
async fn json_errors_and_successes_are_untouched() {
    let (_, _, body) = call("/json").await;
    assert_eq!(body, r#"{"detail":"Server tidak ditemukan"}"#);
    let (status, ct, body) = call("/ok").await;
    assert_eq!(status, StatusCode::OK);
    assert!(ct.starts_with("text/plain"));
    assert_eq!(body, "hello");
}
