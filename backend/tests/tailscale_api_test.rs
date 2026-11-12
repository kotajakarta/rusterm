use axum::body::Body;
use axum::http::{Request, StatusCode};
use tempfile::tempdir;
use tower::ServiceExt;
use xterm_backend::controllers::servers::AppState;
use xterm_backend::controllers::tailscale;
use xterm_backend::models::db::DbStore;
use xterm_backend::services::cloudflare::CloudflareClient;
use xterm_backend::services::ssh_pool::SshPool;
use xterm_backend::services::tailscale::TailscaleClient;
use xterm_backend::services::vault::Vault;

#[tokio::test]
async fn test_tailscale_router_auth_and_crud() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("ts_api_test.sqlite");
    let vault_path = dir.path().join("vault.json");

    let vault = Vault::new(&vault_path).await.unwrap();
    let session = vault.setup_pin("123456").await.unwrap();
    let db = DbStore::new(&db_path).unwrap();
    let pool = SshPool::new();
    let cf = CloudflareClient::new();
    let ts = TailscaleClient::new();

    let app_state = AppState {
        db: db.clone(),
        vault: vault.clone(),
        pool,
        cf,
        ts,
        transfers: Default::default(),
        updater: Default::default(),
    };

    let app = axum::Router::new().nest("/api", tailscale::router(app_state));

    // 1. Unauthorized request
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/tailscale/accounts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 2. Authorized request to list accounts (empty)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/tailscale/accounts")
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
    let accs: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(accs.len(), 0);

    // 3. Create account via POST
    let create_body = serde_json::json!({
        "name": "Work Tailnet",
        "api_key": "tskey-api-k1234567890abcdef",
        "tailnet": "-"
    });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/tailscale/accounts")
                .header("X-Session", &session)
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_vec(&create_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 64)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let acc_id = created["id"].as_i64().unwrap();
    assert_eq!(created["name"], "Work Tailnet");
    assert_eq!(created["tailnet"], "-");
    assert!(created["token_masked"].as_str().unwrap().contains("••••"));

    // 4. Update account via PUT
    let update_body = serde_json::json!({
        "name": "Updated Work Tailnet",
        "tailnet": "workcorp.ts.net"
    });
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/tailscale/accounts/{}", acc_id))
                .header("X-Session", &session)
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_vec(&update_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 5. Delete account via DELETE
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/tailscale/accounts/{}", acc_id))
                .header("X-Session", &session)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Verify empty list again
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/tailscale/accounts")
                .header("X-Session", &session)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(res.into_body(), 1024 * 64)
        .await
        .unwrap();
    let accs_after: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(accs_after.len(), 0);
}
