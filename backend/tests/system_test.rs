use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use tempfile::tempdir;
use tower::ServiceExt;
use xterm_backend::controllers::servers::AppState;
use xterm_backend::controllers::system::{self, is_newer_version, UpdateCheckResult};
use xterm_backend::models::db::DbStore;
use xterm_backend::services::cloudflare::CloudflareClient;
use xterm_backend::services::ssh_pool::SshPool;
use xterm_backend::services::vault::Vault;

#[tokio::test]
async fn test_system_update_check_auth_and_response() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("system_test.sqlite");
    let vault_path = dir.path().join("vault.json");

    let vault = Vault::new(&vault_path).await.unwrap();
    let session = vault.setup_pin("123456").await.unwrap();
    let db = DbStore::new(&db_path).unwrap();
    let pool = SshPool::new();
    let cf = CloudflareClient::new();

    let app_state = AppState {
        db: db.clone(),
        vault: vault.clone(),
        pool,
        cf,
        ts: Default::default(),
        transfers: Default::default(),
        updater: Default::default(),
    };

    let app = axum::Router::new().nest("/api", system::router(app_state));

    // 1. Unauthorized request
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/system/update-check")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 2. Authorized request
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/system/update-check")
                .header("X-Session", &session)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    let check: UpdateCheckResult = serde_json::from_slice(&body).unwrap();

    assert_eq!(check.platform, "desktop");
    assert_eq!(check.current_version, env!("CARGO_PKG_VERSION"));
    assert!(check.download_url.is_some());
    let url = check.download_url.unwrap();
    assert!(url.contains("kotajakarta/rusterm"));

    // 3. Test update-progress endpoint
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/system/update-progress")
                .header("X-Session", &session)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. Test update-apply endpoint
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/system/update-apply")
                .header("X-Session", &session)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[test]
fn test_is_newer_version_semver_comparison() {
    assert!(!is_newer_version("0.2.6", "0.2.6"));
    assert!(!is_newer_version("0.2.6", "v0.2.6"));
    assert!(!is_newer_version("0.2.6", "0.2.5"));
    assert!(!is_newer_version("0.3.0", "0.2.9"));
    assert!(is_newer_version("0.2.6", "0.2.7"));
    assert!(is_newer_version("0.2.6", "v0.3.0"));
    assert!(is_newer_version("0.2.6", "1.0.0"));
    assert!(is_newer_version("0.2.6", "0.2.7-beta.1"));
    assert!(!is_newer_version("0.2.6", "invalid"));
    assert!(!is_newer_version("0.2.6", ""));
}
