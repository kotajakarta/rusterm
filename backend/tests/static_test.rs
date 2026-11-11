use axum::body::Body;
use axum::http::{Request, StatusCode};
use std::path::PathBuf;
use tower::ServiceExt;
use xterm_backend::controllers::static_files::{inject_token_into_html, router, StaticConfig};

#[test]
fn test_inject_token_into_html() {
    let original_html = "<!DOCTYPE html><html><head><title>TermDeck</title></head><body></body></html>";
    let token = "test-session-token-12345";
    let injected = inject_token_into_html(original_html, token);

    assert!(injected.contains(r#"<meta name="termdeck-token" content="test-session-token-12345">"#));
    assert!(injected.contains("<title>TermDeck</title>"));
}

#[tokio::test]
async fn test_embedded_asset_serving_and_token_injection() {
    let app = router(StaticConfig {
        dist_dir: PathBuf::from("/non/existent/path/that/does/not/exist"),
        token: "embedded_test_token_12345".to_string(),
    });

    let res = app
        .oneshot(
            Request::builder()
                .uri("/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(body.contains(r#"<meta name="rusterm-token" content="embedded_test_token_12345">"#));
    assert!(body.contains(r#"<meta name="termdeck-token" content="embedded_test_token_12345">"#));
    assert!(body.contains("<title>RusTerm</title>"));
}

#[tokio::test]
async fn test_unknown_api_route_returns_json_404_not_index_html() {
    let app = router(StaticConfig {
        dist_dir: PathBuf::from("/non/existent/path/that/does/not/exist"),
        token: "test_token".to_string(),
    });

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/servers/1/does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let ct = res.headers().get("content-type").unwrap().to_str().unwrap().to_string();
    assert!(ct.starts_with("application/json"), "content-type: {}", ct);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(json["detail"].is_string());
}

#[tokio::test]
async fn test_path_traversal_is_blocked() {
    let app = router(StaticConfig {
        dist_dir: PathBuf::from("."),
        token: "test_token".to_string(),
    });

    let res = app
        .oneshot(
            Request::builder()
                .uri("/assets/../Cargo.toml")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}
