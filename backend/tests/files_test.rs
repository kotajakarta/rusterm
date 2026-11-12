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
        ts: Default::default(),
        transfers: Default::default(),
        updater: Default::default(),
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

#[tokio::test]
async fn test_upload_accepts_bodies_above_axum_default_limit() {
    let dir = tempdir().unwrap();
    let vault = Vault::new(&dir.path().join("vault.json")).await.unwrap();
    let session = vault.setup_pin("123456").await.unwrap();
    let kek = vault.get_kek().await.unwrap();
    let db = DbStore::new(&dir.path().join("db.sqlite")).unwrap();
    let id = db
        .create_server(
            &ServerInput {
                name: "Unreachable".into(),
                host: "127.0.0.1".into(),
                port: 1,
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
    let app = axum::Router::new().nest(
        "/api",
        files::router(AppState {
            db,
            vault,
            pool: SshPool::new(),
            cf: CloudflareClient::new(),
            ts: Default::default(),
            transfers: Default::default(),
            updater: Default::default(),
        }),
    );

    assert_eq!(files::UPLOAD_MAX_BYTES, 512 * 1024 * 1024);
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/files/{}/upload?dir=/tmp&name=big.bin", id))
                .header("X-Session", &session)
                .header("Content-Type", "application/octet-stream")
                .body(Body::from(vec![0u8; 3 * 1024 * 1024]))
                .unwrap(),
        )
        .await
        .unwrap();
    // Body is accepted; the request then fails only because the SSH port is closed.
    assert_ne!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

async fn transfer_app() -> (tempfile::TempDir, axum::Router, String, i64) {
    let dir = tempdir().unwrap();
    let vault = Vault::new(&dir.path().join("vault.json")).await.unwrap();
    let session = vault.setup_pin("123456").await.unwrap();
    let kek = vault.get_kek().await.unwrap();
    let db = DbStore::new(&dir.path().join("db.sqlite")).unwrap();
    let id = db
        .create_server(
            &ServerInput {
                name: "Unreachable".into(),
                host: "127.0.0.1".into(),
                port: 1,
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
    let app = axum::Router::new().nest(
        "/api",
        files::router(AppState {
            db,
            vault,
            pool: SshPool::new(),
            cf: CloudflareClient::new(),
            ts: Default::default(),
            transfers: Default::default(),
            updater: Default::default(),
        }),
    );
    (dir, app, session, id)
}

async fn send(app: &axum::Router, method: &str, uri: &str, session: &str, body: serde_json::Value) -> StatusCode {
    app.clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("X-Session", session)
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn test_transfer_routes_validate_and_404() {
    let (_dir, app, session, id) = transfer_app().await;
    let none = serde_json::json!({});

    assert_eq!(send(&app, "GET", "/api/files/transfers/abc", "", none.clone()).await, StatusCode::UNAUTHORIZED);
    assert_eq!(send(&app, "GET", "/api/files/transfers/abc", &session, none.clone()).await, StatusCode::NOT_FOUND);
    assert_eq!(
        send(&app, "POST", "/api/files/transfers/abc/cancel", &session, none.clone()).await,
        StatusCode::NOT_FOUND
    );

    let outside = serde_json::json!({
        "source_server_id": id, "dest_server_id": id,
        "source_dir": "/var/www", "dest_dir": "/backup",
        "paths": ["/var/www2/x"], "use_gitignore": true,
    });
    assert_eq!(
        send(&app, "POST", "/api/files/direct-transfer/scan", &session, outside.clone()).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        send(&app, "POST", "/api/files/direct-transfer/start", &session, outside).await,
        StatusCode::BAD_REQUEST
    );

    let body = serde_json::json!({ "direction": "upload", "paths": ["/var/www"] });
    assert_eq!(send(&app, "POST", "/api/files/remotes/999/scan", &session, body.clone()).await, StatusCode::NOT_FOUND);
    assert_eq!(
        send(&app, "POST", "/api/files/remotes/999/transfer", &session, body).await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn test_direct_transfer_rejects_overlapping_source_and_dest_on_same_server() {
    let (_dir, app, session, id) = transfer_app().await;
    for (dest, paths) in [
        ("/srv/app", vec!["/srv/app/data"]),          // same folder: would overwrite itself
        ("/srv/app/data/sub", vec!["/srv/app/data"]), // into itself: endless recursion
    ] {
        let body = serde_json::json!({
            "source_server_id": id, "dest_server_id": id,
            "source_dir": "/srv/app", "dest_dir": dest, "paths": paths, "mode": "move",
        });
        assert_eq!(
            send(&app, "POST", "/api/files/direct-transfer/start", &session, body).await,
            StatusCode::BAD_REQUEST,
            "dest {}",
            dest
        );
    }
}
