use axum::{
    body::{Body, Bytes},
    extract::{Path, Query, State},
    http::{HeaderMap, Request, StatusCode},
    routing::{get, put},
    Router,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tempfile::tempdir;
use tokio::net::TcpListener;
use tower::ServiceExt;
use xterm_backend::controllers::servers::AppState;
use xterm_backend::controllers::storage::{self, StorageAppState};
use xterm_backend::models::db::DbStore;
use xterm_backend::services::cloudflare::CloudflareClient;
use xterm_backend::services::s3::S3Client;
use xterm_backend::services::ssh_pool::SshPool;
use xterm_backend::services::storage_transfer::StorageTransferManager;
use xterm_backend::services::tailscale::TailscaleClient;
use xterm_backend::services::vault::Vault;

#[derive(Default)]
struct MockS3 {
    buckets: Vec<String>,
    objects: HashMap<(String, String), Vec<u8>>,
}

async fn spawn_mock_s3(st: Arc<Mutex<MockS3>>) -> String {
    let app = Router::new()
        .route(
            "/",
            get(|State(st): State<Arc<Mutex<MockS3>>>| async move {
                let g = st.lock().unwrap();
                let mut xml = String::from("<ListAllMyBucketsResult><Buckets>");
                for b in &g.buckets {
                    xml.push_str(&format!("<Bucket><Name>{}</Name><CreationDate>2026-01-01T00:00:00Z</CreationDate></Bucket>", b));
                }
                xml.push_str("</Buckets></ListAllMyBucketsResult>");
                (StatusCode::OK, xml)
            }),
        )
        .route(
            "/:bucket",
            put(|State(st): State<Arc<Mutex<MockS3>>>, Path(b): Path<String>| async move {
                let mut g = st.lock().unwrap();
                if !g.buckets.contains(&b) {
                    g.buckets.push(b);
                }
                StatusCode::OK
            })
            .delete(|State(st): State<Arc<Mutex<MockS3>>>, Path(b): Path<String>| async move {
                st.lock().unwrap().buckets.retain(|x| x != &b);
                StatusCode::NO_CONTENT
            })
            .get(
                |State(st): State<Arc<Mutex<MockS3>>>,
                 Path(bucket): Path<String>,
                 Query(params): Query<HashMap<String, String>>| async move {
                    let g = st.lock().unwrap();
                    let prefix = params.get("prefix").cloned().unwrap_or_default();
                    let mut contents = String::new();
                    for ((b, k), data) in &g.objects {
                        if b == &bucket && k.starts_with(&prefix) {
                            contents.push_str(&format!(
                                "<Contents><Key>{}</Key><LastModified>2026-01-01T00:00:00Z</LastModified><ETag>\"e\"</ETag><Size>{}</Size></Contents>",
                                k,
                                data.len()
                            ));
                        }
                    }
                    let xml = format!(
                        "<ListBucketResult><Name>{}</Name><Prefix>{}</Prefix><IsTruncated>false</IsTruncated>{}</ListBucketResult>",
                        bucket, prefix, contents
                    );
                    (StatusCode::OK, xml)
                },
            ),
        )
        .route(
            "/:bucket/*key",
            put(
                |State(st): State<Arc<Mutex<MockS3>>>,
                 Path((b, k)): Path<(String, String)>,
                 headers: HeaderMap,
                 body: Bytes| async move {
                    let mut g = st.lock().unwrap();
                    if let Some(cs) = headers.get("x-amz-copy-source").and_then(|v| v.to_str().ok()) {
                        let trimmed = cs.trim_start_matches('/');
                        if let Some((sb, sk)) = trimmed.split_once('/') {
                            let data = g.objects.get(&(sb.to_string(), sk.to_string())).cloned().unwrap_or_default();
                            g.objects.insert((b, k), data);
                        }
                        return (StatusCode::OK, "<CopyObjectResult/>");
                    }
                    g.objects.insert((b, k), body.to_vec());
                    (StatusCode::OK, "")
                },
            )
            .get(
                |State(st): State<Arc<Mutex<MockS3>>>, Path((b, k)): Path<(String, String)>| async move {
                    match st.lock().unwrap().objects.get(&(b, k)).cloned() {
                        Some(d) => (StatusCode::OK, d),
                        None => (StatusCode::NOT_FOUND, b"NotFound".to_vec()),
                    }
                },
            )
            .delete(
                |State(st): State<Arc<Mutex<MockS3>>>, Path((b, k)): Path<(String, String)>| async move {
                    st.lock().unwrap().objects.remove(&(b, k));
                    StatusCode::NO_CONTENT
                },
            ),
        )
        .with_state(st);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{}", addr)
}

async fn setup_app() -> (Router, String, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let vault = Vault::new(&dir.path().join("vault.json")).await.unwrap();
    let session = vault.setup_pin("123456").await.unwrap();
    let db = DbStore::new(&dir.path().join("db.sqlite")).unwrap();
    let app_state = AppState {
        db,
        vault,
        pool: SshPool::new(),
        cf: CloudflareClient::new(),
        ts: TailscaleClient::new(),
        transfers: Default::default(),
        updater: Default::default(),
    };
    let state = StorageAppState {
        app: app_state,
        s3: S3Client::new(),
        storage_transfers: StorageTransferManager::new(),
    };
    (storage::router(state), session, dir)
}

#[tokio::test]
async fn test_storage_api_auth_and_end_to_end() {
    let (app, session, _dir) = setup_app().await;
    let mock_st = Arc::new(Mutex::new(MockS3::default()));
    let endpoint = spawn_mock_s3(mock_st.clone()).await;

    // 1. Unauthenticated request is rejected with 401
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/storage/accounts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // 2. Create account (rejects bad scheme)
    let bad_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/storage/accounts")
                .header("X-Session", &session)
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Bad",
                        "provider": "minio",
                        "endpoint": "file:///etc/passwd",
                        "region": "us-east-1",
                        "access_key_id": "a",
                        "secret_access_key": "s",
                        "path_style": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad_resp.status(), StatusCode::BAD_REQUEST);

    // 3. Create valid account
    let create_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/storage/accounts")
                .header("X-Session", &session)
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Local MinIO",
                        "provider": "minio",
                        "endpoint": endpoint,
                        "region": "us-east-1",
                        "access_key_id": "minioadmin",
                        "secret_access_key": "miniosecret",
                        "path_style": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(create_resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let acc: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let acc_id = acc["id"].as_i64().unwrap();
    assert!(acc.get("secret_access_key").is_none());

    // 4. Create bucket
    let b_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/storage/accounts/{}/buckets", acc_id))
                .header("X-Session", &session)
                .header("Content-Type", "application/json")
                .body(Body::from(json!({ "name": "test-bucket" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(b_resp.status(), StatusCode::OK);

    // 5. Create folder & presign
    let f_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/storage/accounts/{}/buckets/test-bucket/folder", acc_id))
                .header("X-Session", &session)
                .header("Content-Type", "application/json")
                .body(Body::from(json!({ "prefix": "docs" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(f_resp.status(), StatusCode::OK);

    let p_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/storage/accounts/{}/buckets/test-bucket/presign", acc_id))
                .header("X-Session", &session)
                .header("Content-Type", "application/json")
                .body(Body::from(json!({ "key": "docs/a.txt", "expires_secs": 600 }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(p_resp.status(), StatusCode::OK);
    let p_body = axum::body::to_bytes(p_resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let p_json: serde_json::Value = serde_json::from_slice(&p_body).unwrap();
    assert!(p_json["url"].as_str().unwrap().contains("X-Amz-Signature="));
}
