use axum::body::Body;
use axum::http::{Request, StatusCode};
use tempfile::tempdir;
use tower::ServiceExt;
use xterm_backend::controllers::files;
use xterm_backend::controllers::servers::AppState;
use xterm_backend::models::db::DbStore;
use xterm_backend::models::files::FileRemoteInput;
use xterm_backend::models::server::ServerInput;
use xterm_backend::services::cloudflare::CloudflareClient;
use xterm_backend::services::ssh_pool::SshPool;
use xterm_backend::services::vault::Vault;

#[tokio::test]
async fn test_files_router_auth_and_remotes() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("files_test.sqlite");
    let vault_path = dir.path().join("vault.json");

    let vault = Vault::new(&vault_path).await.unwrap();
    let session = vault.setup_pin("123456").await.unwrap();
    let kek = vault.get_kek().await.unwrap();
    let db = DbStore::new(&db_path).unwrap();
    let pool = SshPool::new();

    let s1 = db
        .create_server(
            &ServerInput {
                name: "Web1".into(),
                host: "1.1.1.1".into(),
                port: 22,
                username: "root".into(),
                auth_type: "password".into(),
                password: Some("pass".into()),
                private_key: None,
                passphrase: None,
                group_name: "".into(),
                color: "".into(),
            },
            &kek,
        )
        .unwrap();

    let app_state = AppState {
        db: db.clone(),
        vault: vault.clone(),
        pool,
        cf: CloudflareClient::new(),
    };

    let app = axum::Router::new().nest("/api", files::router(app_state));

    // 1. Unauthorized request
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/files/remotes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 2. Authorized request to list remotes (empty initially)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/files/remotes")
                .header("X-Session", &session)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 3. Create remote via API
    let body = serde_json::to_string(&FileRemoteInput {
        name: "Sync HTML".into(),
        local_server_id: s1,
        local_path: "/var/www/html".into(),
        remote_server_id: s1,
        remote_path: "/backup/html".into(),
        ignore: vec![".git".into()],
    })
    .unwrap();

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/files/remotes")
                .header("X-Session", &session)
                .header("Content-Type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // 4. List file bookmarks
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/files/bookmarks?server_id={}", s1))
                .header("X-Session", &session)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 5. Toggle bookmark
    let body = serde_json::json!({
        "server_id": s1,
        "path": "/etc/hosts"
    })
    .to_string();

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/files/bookmarks/toggle")
                .header("X-Session", &session)
                .header("Content-Type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
