use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use tempfile::tempdir;
use tower::ServiceExt;
use xterm_backend::controllers::db_client::{self, DbClientAppState};
use xterm_backend::controllers::servers::AppState;
use xterm_backend::models::db::DbStore;
use xterm_backend::models::db_client::{DbConnectionInput, DbEngine, SslMode};
use xterm_backend::services::cloudflare::CloudflareClient;
use xterm_backend::services::db_drivers::{
    quote_ident, validate_column_type, validate_default_expr,
};
use xterm_backend::services::db_pool::DbClientPool;
use xterm_backend::services::ssh_pool::SshPool;
use xterm_backend::services::tailscale::TailscaleClient;
use xterm_backend::services::vault::Vault;

async fn setup_app() -> (axum::Router, String, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let vault_path = dir.path().join("vault.json");
    let db_path = dir.path().join("xterm.sqlite");

    let vault = Vault::new(&vault_path).await.unwrap();
    let session = vault.setup_pin("123456").await.unwrap();
    let db = DbStore::new(&db_path).unwrap();

    let app_state = AppState {
        db,
        vault,
        pool: SshPool::new(),
        cf: CloudflareClient::new(),
        ts: TailscaleClient::new(),
        transfers: Default::default(),
        updater: Default::default(),
    };
    let router = db_client::router(DbClientAppState {
        app: app_state,
        db_pool: DbClientPool::new(),
    });
    (router, session, dir)
}

async fn send_json(
    router: &axum::Router,
    method: &str,
    uri: &str,
    session: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(s) = session {
        builder = builder.header("X-Session", s);
    }
    let req = if let Some(b) = body {
        builder
            .header("Content-Type", "application/json")
            .body(Body::from(b.to_string()))
            .unwrap()
    } else {
        builder.body(Body::empty()).unwrap()
    };

    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let val = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()))
    };
    (status, val)
}

#[test]
fn identifier_and_ddl_validators_reject_injection() {
    assert_eq!(quote_ident(DbEngine::Postgres, "users").unwrap(), "\"users\"");
    assert_eq!(
        quote_ident(DbEngine::Postgres, "a\"b").unwrap(),
        "\"a\"\"b\""
    );
    assert_eq!(quote_ident(DbEngine::Mysql, "a`b").unwrap(), "`a``b`");
    assert!(quote_ident(DbEngine::Sqlite, "").is_err());
    assert!(quote_ident(DbEngine::Sqlite, "bad\0name").is_err());

    assert!(validate_column_type("VARCHAR(255)").is_ok());
    assert!(validate_column_type("DECIMAL(10, 2)").is_ok());
    assert!(validate_column_type("INTEGER; DROP TABLE users").is_err());

    assert!(validate_default_expr("0").is_ok());
    assert!(validate_default_expr("CURRENT_TIMESTAMP").is_ok());
    assert!(validate_default_expr("0; DROP TABLE users").is_err());
    assert!(validate_default_expr("0 -- comment").is_err());
}

#[tokio::test]
async fn db_connection_password_encrypted_and_survives_kek_rotation() {
    let dir = tempdir().unwrap();
    let db = DbStore::new(&dir.path().join("xterm.sqlite")).unwrap();
    let old_kek = [7u8; 32];
    let new_kek = [9u8; 32];

    let id = db
        .create_db_connection(
            &DbConnectionInput {
                name: "Prod PG".into(),
                engine: DbEngine::Postgres,
                host: "127.0.0.1".into(),
                port: 5432,
                username: "admin".into(),
                password: Some("super-secret-pw".into()),
                default_database: "app".into(),
                file_path: String::new(),
                ssl_mode: SslMode::Prefer,
                ssh_server_id: None,
                read_only: true,
                color: "#22c55e".into(),
            },
            &old_kek,
        )
        .unwrap();

    let out = db.get_db_connection(id).unwrap();
    assert!(out.has_password);
    assert!(out.read_only);

    let dec_before = db.get_db_connection_decrypted(id, &old_kek).unwrap();
    assert_eq!(dec_before.password.as_deref(), Some("super-secret-pw"));

    db.rotate_kek(&old_kek, &new_kek).unwrap();
    assert!(db.get_db_connection_decrypted(id, &old_kek).is_err());
    let dec_after = db.get_db_connection_decrypted(id, &new_kek).unwrap();
    assert_eq!(dec_after.password.as_deref(), Some("super-secret-pw"));
}

#[tokio::test]
async fn sqlite_full_lifecycle_and_read_only_guard() {
    let (router, session, dir) = setup_app().await;
    let target_sqlite = dir.path().join("sample.sqlite");
    let target_path = target_sqlite.to_string_lossy().to_string();

    // Unauthorized tanpa session
    let (st, _) = send_json(&router, "GET", "/db/connections", None, None).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);

    // Create SQLite connection
    let (st, created) = send_json(
        &router,
        "POST",
        "/db/connections",
        Some(&session),
        Some(json!({
            "name": "Local SQLite",
            "engine": "sqlite",
            "file_path": target_path,
            "read_only": false
        })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "resp: {}", created);
    let conn_id = created["id"].as_i64().unwrap();

    // Test connection
    let (st, test_res) = send_json(
        &router,
        "POST",
        "/db/connections/test",
        Some(&session),
        Some(json!({
            "id": conn_id,
            "engine": "sqlite",
            "file_path": target_path
        })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "resp: {}", test_res);
    assert_eq!(test_res["ok"], Value::Bool(true));

    // Create table
    let (st, body) = send_json(
        &router,
        "POST",
        &format!("/db/{}/tables", conn_id),
        Some(&session),
        Some(json!({
            "db": "main",
            "table": "items",
            "columns": [
                { "name": "id", "data_type": "INTEGER", "nullable": false, "primary_key": true },
                { "name": "title", "data_type": "TEXT", "nullable": false, "primary_key": false },
                { "name": "price", "data_type": "INTEGER", "nullable": true, "primary_key": false }
            ]
        })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "resp: {}", body);

    // List schemas on SQLite returns empty array, create/drop schema returns unsupported (400)
    let (st, schemas) = send_json(
        &router,
        "GET",
        &format!("/db/{}/schemas?db=main", conn_id),
        Some(&session),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(schemas, json!([]));

    let (st, _) = send_json(
        &router,
        "POST",
        &format!("/db/{}/schemas", conn_id),
        Some(&session),
        Some(json!({ "db": "main", "name": "analytics" })),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);

    let (st, _) = send_json(
        &router,
        "DELETE",
        &format!("/db/{}/schemas/analytics?db=main", conn_id),
        Some(&session),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);

    // List tables & describe schema
    let (st, tables) = send_json(
        &router,
        "GET",
        &format!("/db/{}/tables?db=main&schema=public", conn_id),
        Some(&session),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(tables.as_array().unwrap().len(), 1);
    assert_eq!(tables[0]["name"], "items");

    let (st, schema) = send_json(
        &router,
        "GET",
        &format!("/db/{}/tables/items/schema?db=main", conn_id),
        Some(&session),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(schema["primary_keys"], json!(["id"]));

    // Insert 2 rows
    for (id, title, price) in [(1, "Kopi", 15000), (2, "Teh", 10000)] {
        let (st, res) = send_json(
            &router,
            "POST",
            &format!("/db/{}/tables/items/rows", conn_id),
            Some(&session),
            Some(json!({
                "db": "main",
                "op": "insert",
                "values": { "id": id, "title": title, "price": price }
            })),
        )
        .await;
        assert_eq!(st, StatusCode::OK, "insert resp: {}", res);
        assert_eq!(res["affected_rows"], 1);
    }

    // Fetch rows with sort & filter
    let (st, page) = send_json(
        &router,
        "GET",
        &format!("/db/{}/tables/items/rows?db=main&sort=price&dir=asc&filter_col=title&filter_op=contains&filter_val=eh", conn_id),
        Some(&session),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "page: {}", page);
    assert_eq!(page["rows"].as_array().unwrap().len(), 1);
    assert_eq!(page["rows"][0][1]["value"]["value"], "Teh");

    // Update row
    let (st, upd) = send_json(
        &router,
        "POST",
        &format!("/db/{}/tables/items/rows", conn_id),
        Some(&session),
        Some(json!({
            "db": "main",
            "op": "update",
            "pk": { "id": 1 },
            "values": { "price": 18000 }
        })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "upd: {}", upd);
    assert_eq!(upd["affected_rows"], 1);

    // Query runner
    let (st, qres) = send_json(
        &router,
        "POST",
        &format!("/db/{}/query", conn_id),
        Some(&session),
        Some(json!({ "db": "main", "sql": "SELECT title, price FROM items WHERE id = 1" })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "qres: {}", qres);
    assert_eq!(qres["rows"][0][1]["value"], 18000);

    // Delete row
    let (st, del) = send_json(
        &router,
        "POST",
        &format!("/db/{}/tables/items/rows", conn_id),
        Some(&session),
        Some(json!({
            "db": "main",
            "op": "delete",
            "pk": { "id": 2 }
        })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "del: {}", del);
    assert_eq!(del["affected_rows"], 1);

    // Switch connection to Read-Only and verify mutations are blocked
    let (st, _) = send_json(
        &router,
        "PUT",
        &format!("/db/connections/{}", conn_id),
        Some(&session),
        Some(json!({
            "name": "Local SQLite RO",
            "engine": "sqlite",
            "file_path": target_path,
            "read_only": true
        })),
    )
    .await;
    assert_eq!(st, StatusCode::OK);

    let (st, _) = send_json(
        &router,
        "POST",
        &format!("/db/{}/tables/items/rows", conn_id),
        Some(&session),
        Some(json!({
            "db": "main",
            "op": "delete",
            "pk": { "id": 1 }
        })),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);

    let (st, _) = send_json(
        &router,
        "POST",
        &format!("/db/{}/query", conn_id),
        Some(&session),
        Some(json!({ "db": "main", "sql": "DELETE FROM items" })),
    )
    .await;
    assert!(st == StatusCode::FORBIDDEN || st == StatusCode::BAD_REQUEST);

    let (st, _) = send_json(
        &router,
        "DELETE",
        &format!("/db/{}/tables/items?db=main", conn_id),
        Some(&session),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);

    let (st, _) = send_json(
        &router,
        "POST",
        &format!("/db/{}/schemas", conn_id),
        Some(&session),
        Some(json!({ "db": "main", "name": "analytics" })),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);

    let (st, _) = send_json(
        &router,
        "DELETE",
        &format!("/db/{}/schemas/analytics?db=main", conn_id),
        Some(&session),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn mysql_tls_connection_does_not_panic_on_rustls_provider() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    // Mock MariaDB/MySQL server advertising CLIENT_SSL in initial handshake v10
    let server_task = tokio::spawn(async move {
        for _ in 0..2 {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let mut payload = Vec::new();
            payload.push(0x0a); // protocol v10
            payload.extend_from_slice(b"10.11.6-MariaDB\0");
            payload.extend_from_slice(&1u32.to_le_bytes()); // conn id
            payload.extend_from_slice(b"12345678"); // auth plugin data part 1
            payload.push(0x00); // filler
            payload.extend_from_slice(&0x8a00u16.to_le_bytes()); // PROTOCOL_41 | SSL | SECURE_CONN
            payload.push(0x21); // charset utf8
            payload.extend_from_slice(&0x0002u16.to_le_bytes()); // status
            payload.extend_from_slice(&0x0008u16.to_le_bytes()); // CLIENT_PLUGIN_AUTH
            payload.push(21); // auth plugin data len
            payload.extend_from_slice(&[0u8; 10]); // reserved
            payload.extend_from_slice(b"123456789012\0"); // auth plugin data part 2
            payload.extend_from_slice(b"mysql_native_password\0");

            let len = payload.len() as u32;
            let mut pkt = vec![
                (len & 0xff) as u8,
                ((len >> 8) & 0xff) as u8,
                ((len >> 16) & 0xff) as u8,
                0x00,
            ];
            pkt.extend_from_slice(&payload);
            let _ = socket.write_all(&pkt).await;

            // Read client SSLRequest or handshake response then close cleanly
            let mut buf = [0u8; 256];
            let _ = socket.read(&mut buf).await;
        }
    });

    let (router, session, _dir) = setup_app().await;
    let (st, body) = send_json(
        &router,
        "POST",
        "/db/connections/test",
        Some(&session),
        Some(json!({
            "engine": "mysql",
            "host": "127.0.0.1",
            "port": port,
            "username": "root",
            "password": "!secretPassword123",
            "default_database": "auth-doc",
            "ssl_mode": "prefer"
        })),
    )
    .await;

    server_task.abort();
    // Harus mengembalikan error HTTP (502 Bad Gateway), bukan panic / ERR_EMPTY_RESPONSE
    assert_eq!(st, StatusCode::BAD_GATEWAY, "unexpected body: {}", body);
}
