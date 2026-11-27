use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, put},
    Router,
};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use xterm_backend::models::storage::DecryptedStorageAccount;
use xterm_backend::services::s3::S3Client;
use xterm_backend::services::storage_transfer::{
    sanitize_relative_key, StorageTransferManager, StorageTransferSpec,
};

#[derive(Default)]
struct MockS3State {
    objects: HashMap<(String, String), Vec<u8>>,
    copy_calls: usize,
}

async fn spawn_mock_s3(state: Arc<Mutex<MockS3State>>) -> String {
    let app = Router::new()
        .route(
            "/:bucket",
            get(
                |State(st): State<Arc<Mutex<MockS3State>>>,
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
                        r#"<?xml version="1.0" encoding="UTF-8"?><ListBucketResult><Name>{}</Name><Prefix>{}</Prefix><IsTruncated>false</IsTruncated>{}</ListBucketResult>"#,
                        bucket, prefix, contents
                    );
                    (StatusCode::OK, xml)
                },
            ),
        )
        .route(
            "/:bucket/*key",
            put(
                |State(st): State<Arc<Mutex<MockS3State>>>,
                 Path((bucket, key)): Path<(String, String)>,
                 headers: HeaderMap,
                 body: Bytes| async move {
                    let mut g = st.lock().unwrap();
                    if let Some(copy_src) =
                        headers.get("x-amz-copy-source").and_then(|v| v.to_str().ok())
                    {
                        g.copy_calls += 1;
                        let trimmed = copy_src.trim_start_matches('/');
                        if let Some((sb, sk)) = trimmed.split_once('/') {
                            let data = g
                                .objects
                                .get(&(sb.to_string(), sk.to_string()))
                                .cloned()
                                .unwrap_or_default();
                            g.objects.insert((bucket, key), data);
                        }
                        return (StatusCode::OK, "<CopyObjectResult/>");
                    }
                    g.objects.insert((bucket, key), body.to_vec());
                    (StatusCode::OK, "")
                },
            )
            .get(
                |State(st): State<Arc<Mutex<MockS3State>>>,
                 Path((bucket, key)): Path<(String, String)>| async move {
                    let g = st.lock().unwrap();
                    match g.objects.get(&(bucket, key)) {
                        Some(data) => (StatusCode::OK, data.clone()),
                        None => (StatusCode::NOT_FOUND, b"NotFound".to_vec()),
                    }
                },
            ),
        )
        .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{}", addr)
}

fn acc(id: i64, endpoint: &str) -> DecryptedStorageAccount {
    DecryptedStorageAccount {
        id,
        name: format!("Acc {}", id),
        provider: "minio".into(),
        endpoint: endpoint.into(),
        region: "us-east-1".into(),
        access_key_id: "AKIA".into(),
        secret_access_key: "SECRET".into(),
        default_bucket: String::new(),
        path_style: true,
    }
}

#[test]
fn test_sanitize_relative_key_blocks_traversal() {
    assert_eq!(sanitize_relative_key("a/b/c.txt").unwrap(), "a/b/c.txt");
    assert_eq!(sanitize_relative_key("/leading/ok.txt").unwrap(), "leading/ok.txt");
    assert!(sanitize_relative_key("../etc/passwd").is_err());
    assert!(sanitize_relative_key("foo/../../bar").is_err());
    assert!(sanitize_relative_key("").is_err());
}

#[tokio::test]
async fn test_s3_to_s3_same_account_server_copy_and_cross_account_relay() {
    let st1 = Arc::new(Mutex::new(MockS3State::default()));
    st1.lock()
        .unwrap()
        .objects
        .insert(("src-b".into(), "backup/db.sql".into()), b"SELECT 1;".to_vec());
    st1.lock()
        .unwrap()
        .objects
        .insert(("src-b".into(), "backup/meta.json".into()), b"{}".to_vec());
    let ep1 = spawn_mock_s3(st1.clone()).await;

    let st2 = Arc::new(Mutex::new(MockS3State::default()));
    let ep2 = spawn_mock_s3(st2.clone()).await;

    let s3 = S3Client::new();
    let mgr = StorageTransferManager::new();

    // 1. Same-account copy across buckets -> server_copy
    let job_same = mgr.start_s3_to_s3(
        s3.clone(),
        StorageTransferSpec {
            direction: "s3_to_s3".into(),
            mode: "auto".into(),
            overwrite: true,
        },
        acc(1, &ep1),
        "src-b".into(),
        "backup/".into(),
        acc(1, &ep1),
        "dst-b".into(),
        "archive/".into(),
    );
    let final_same = mgr.wait_finished(&job_same.id).await.unwrap();
    assert_eq!(final_same.state, "done", "error: {:?}", final_same.error);
    assert_eq!(final_same.effective_mode, "server_copy");
    assert_eq!(final_same.transferred_files, 2);
    assert_eq!(st1.lock().unwrap().copy_calls, 2);
    assert_eq!(
        st1.lock().unwrap().objects.get(&("dst-b".into(), "archive/db.sql".into())),
        Some(&b"SELECT 1;".to_vec())
    );

    // 2. Cross-account copy -> relay streaming
    let job_cross = mgr.start_s3_to_s3(
        s3.clone(),
        StorageTransferSpec {
            direction: "s3_to_s3".into(),
            mode: "auto".into(),
            overwrite: true,
        },
        acc(1, &ep1),
        "src-b".into(),
        "backup/".into(),
        acc(2, &ep2),
        "r2-b".into(),
        "mirror/".into(),
    );
    let final_cross = mgr.wait_finished(&job_cross.id).await.unwrap();
    assert_eq!(final_cross.state, "done", "error: {:?}", final_cross.error);
    assert_eq!(final_cross.effective_mode, "relay");
    assert_eq!(final_cross.transferred_files, 2);
    assert_eq!(
        st2.lock().unwrap().objects.get(&("r2-b".into(), "mirror/db.sql".into())),
        Some(&b"SELECT 1;".to_vec())
    );
}

#[tokio::test]
async fn test_files_agent_s3_walk_and_http_put_get() {
    let st = Arc::new(Mutex::new(MockS3State::default()));
    let ep = spawn_mock_s3(st.clone()).await;

    let tmp = tempfile::tempdir().unwrap();
    let src_dir = tmp.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    let src_file = src_dir.join("hello.txt");
    std::fs::write(&src_file, b"agent-payload-123").unwrap();

    let agent_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/files_agent.py");

    // 1. s3_walk
    let walk_req = serde_json::json!({
        "op": "s3_walk",
        "path": src_dir.to_string_lossy(),
    });
    let mut child = tokio::process::Command::new("python3")
        .arg(&agent_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        use tokio::io::AsyncWriteExt;
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(walk_req.to_string().as_bytes())
            .await
            .unwrap();
    }
    let out = child.wait_with_output().await.unwrap();
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["result"]["files"].as_array().unwrap().len(), 1);
    assert_eq!(v["result"]["files"][0]["rel"], "hello.txt");

    // 2. http_put_file
    let put_url = format!("{}/my-b/hello.txt", ep);
    let put_req = serde_json::json!({
        "op": "http_put_file",
        "path": src_file.to_string_lossy(),
        "url": put_url,
    });
    let mut child2 = tokio::process::Command::new("python3")
        .arg(&agent_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        use tokio::io::AsyncWriteExt;
        child2
            .stdin
            .as_mut()
            .unwrap()
            .write_all(put_req.to_string().as_bytes())
            .await
            .unwrap();
    }
    let out2 = child2.wait_with_output().await.unwrap();
    assert!(out2.status.success(), "stderr: {}", String::from_utf8_lossy(&out2.stderr));
    assert_eq!(
        st.lock().unwrap().objects.get(&("my-b".into(), "hello.txt".into())),
        Some(&b"agent-payload-123".to_vec())
    );

    // 3. http_get_file
    let dst_file = tmp.path().join("downloaded/hello.txt");
    let get_req = serde_json::json!({
        "op": "http_get_file",
        "path": dst_file.to_string_lossy(),
        "url": format!("{}/my-b/hello.txt", ep),
    });
    let mut child3 = tokio::process::Command::new("python3")
        .arg(&agent_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        use tokio::io::AsyncWriteExt;
        child3
            .stdin
            .as_mut()
            .unwrap()
            .write_all(get_req.to_string().as_bytes())
            .await
            .unwrap();
    }
    let out3 = child3.wait_with_output().await.unwrap();
    assert!(out3.status.success(), "stderr: {}", String::from_utf8_lossy(&out3.stderr));
    assert_eq!(std::fs::read(&dst_file).unwrap(), b"agent-payload-123");
}
