use axum::body::Body;
use axum::http::{Request, StatusCode};
use tempfile::tempdir;
use tower::ServiceExt;
use xterm_backend::controllers::cloudflare;
use xterm_backend::controllers::servers::AppState;
use xterm_backend::models::cloudflare::CFAccount;
use xterm_backend::models::db::DbStore;
use xterm_backend::services::cloudflare::CloudflareClient;
use xterm_backend::services::ssh_pool::SshPool;
use xterm_backend::services::vault::Vault;

#[tokio::test]
async fn test_cloudflare_router_auth_and_status() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("cf_test.sqlite");
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
    };

    let app = axum::Router::new().nest("/api", cloudflare::router(app_state));

    // 1. Unauthorized request
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/cf/accounts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 2. Authorized request to list accounts (empty initially)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/cf/accounts")
                .header("X-Session", &session)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 3. Sync status with no accounts configured
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/cf/sync/status")
                .header("X-Session", &session)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 64)
        .await
        .unwrap();
    let status_json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(status_json["configured"], false);
    assert_eq!(status_json["has_remote"], false);

    // 4. Create an account directly in DB and verify list
    let acc: CFAccount = db
        .create_cf_account("Test CF", "dummy_token_12345678", "cf_acc_1", "Test Org")
        .unwrap();
    assert_eq!(acc.name, "Test CF");

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/cf/accounts")
                .header("X-Session", &session)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 64)
        .await
        .unwrap();
    let accs: Vec<CFAccount> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(accs.len(), 1);
    assert_eq!(accs[0].id, acc.id);
    assert!(accs[0].token_masked.contains("••••"));
}
