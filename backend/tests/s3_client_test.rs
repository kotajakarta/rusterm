use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, put},
    Router,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use xterm_backend::models::storage::DecryptedStorageAccount;
use xterm_backend::services::s3::{
    validate_bucket_name, validate_endpoint_url, S3Client,
};

#[derive(Default)]
struct MockS3State {
    buckets: Vec<String>,
    objects: HashMap<(String, String), Vec<u8>>,
    forbid_list_buckets: bool,
    last_auth_header: Option<String>,
    last_copy_source: Option<String>,
}

async fn spawn_mock_s3(state: Arc<Mutex<MockS3State>>) -> String {
    let app = Router::new()
        .route(
            "/",
            get(
                |State(st): State<Arc<Mutex<MockS3State>>>, headers: HeaderMap| async move {
                    let mut g = st.lock().unwrap();
                    g.last_auth_header = headers
                        .get("authorization")
                        .and_then(|v| v.to_str().ok())
                        .map(|s| s.to_string());
                    if g.forbid_list_buckets {
                        return (StatusCode::FORBIDDEN, "<Error><Code>AccessDenied</Code></Error>".to_string());
                    }
                    let mut xml = String::from(
                        r#"<?xml version="1.0" encoding="UTF-8"?><ListAllMyBucketsResult><Buckets>"#,
                    );
                    for b in &g.buckets {
                        xml.push_str(&format!(
                            "<Bucket><Name>{}</Name><CreationDate>2026-01-01T00:00:00Z</CreationDate></Bucket>",
                            b
                        ));
                    }
                    xml.push_str("</Buckets></ListAllMyBucketsResult>");
                    (StatusCode::OK, xml)
                },
            ),
        )
        .route(
            "/:bucket",
            put(
                |State(st): State<Arc<Mutex<MockS3State>>>, Path(bucket): Path<String>| async move {
                    let mut g = st.lock().unwrap();
                    if !g.buckets.contains(&bucket) {
                        g.buckets.push(bucket);
                    }
                    StatusCode::OK
                },
            )
            .delete(
                |State(st): State<Arc<Mutex<MockS3State>>>, Path(bucket): Path<String>| async move {
                    let mut g = st.lock().unwrap();
                    g.buckets.retain(|b| b != &bucket);
                    StatusCode::NO_CONTENT
                },
            )
            .get(
                |State(st): State<Arc<Mutex<MockS3State>>>,
                 Path(bucket): Path<String>,
                 Query(params): Query<HashMap<String, String>>| async move {
                    let g = st.lock().unwrap();
                    let prefix = params.get("prefix").cloned().unwrap_or_default();
                    let delimiter = params.get("delimiter").cloned().unwrap_or_default();
                    let mut folders: Vec<String> = Vec::new();
                    let mut contents = String::new();
                    for ((b, k), data) in &g.objects {
                        if b != &bucket || !k.starts_with(&prefix) {
                            continue;
                        }
                        let rest = &k[prefix.len()..];
                        if !delimiter.is_empty() {
                            if let Some(pos) = rest.find(&delimiter) {
                                let fp = format!("{}{}{}", prefix, &rest[..pos], delimiter);
                                if !folders.contains(&fp) {
                                    folders.push(fp);
                                }
                                continue;
                            }
                        }
                        contents.push_str(&format!(
                            "<Contents><Key>{}</Key><LastModified>2026-01-01T00:00:00Z</LastModified><ETag>\"abc\"</ETag><Size>{}</Size></Contents>",
                            k,
                            data.len()
                        ));
                    }
                    let mut prefixes_xml = String::new();
                    for f in folders {
                        prefixes_xml.push_str(&format!("<CommonPrefixes><Prefix>{}</Prefix></CommonPrefixes>", f));
                    }
                    let xml = format!(
                        r#"<?xml version="1.0" encoding="UTF-8"?><ListBucketResult><Name>{}</Name><Prefix>{}</Prefix><IsTruncated>false</IsTruncated>{}{}</ListBucketResult>"#,
                        bucket, prefix, contents, prefixes_xml
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
                    if let Some(copy_src) = headers.get("x-amz-copy-source").and_then(|v| v.to_str().ok()) {
                        g.last_copy_source = Some(copy_src.to_string());
                        let trimmed = copy_src.trim_start_matches('/');
                        if let Some((sb, sk)) = trimmed.split_once('/') {
                            let decoded_sk = url::form_urlencoded::parse(format!("k={}", sk).as_bytes())
                                .next()
                                .map(|(_, v)| v.into_owned())
                                .unwrap_or_else(|| sk.to_string());
                            let data = g
                                .objects
                                .get(&(sb.to_string(), decoded_sk))
                                .cloned()
                                .unwrap_or_default();
                            g.objects.insert((bucket, key), data);
                        }
                        return (
                            StatusCode::OK,
                            "<CopyObjectResult><ETag>\"copied\"</ETag></CopyObjectResult>",
                        );
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
            )
            .delete(
                |State(st): State<Arc<Mutex<MockS3State>>>,
                 Path((bucket, key)): Path<(String, String)>| async move {
                    let mut g = st.lock().unwrap();
                    g.objects.remove(&(bucket, key));
                    StatusCode::NO_CONTENT
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

fn make_account(endpoint: &str) -> DecryptedStorageAccount {
    DecryptedStorageAccount {
        id: 1,
        name: "Mock S3".into(),
        provider: "minio".into(),
        endpoint: endpoint.into(),
        region: "us-east-1".into(),
        access_key_id: "AKIAEXAMPLE".into(),
        secret_access_key: "SECRETEXAMPLEKEY".into(),
        default_bucket: "scoped-bucket".into(),
        path_style: true,
    }
}

#[test]
fn test_validators_and_presign_url() {
    assert!(validate_endpoint_url("https://acct.r2.cloudflarestorage.com").is_ok());
    assert!(validate_endpoint_url("http://127.0.0.1:9000").is_ok());
    assert!(validate_endpoint_url("file:///etc/passwd").is_err());
    assert!(validate_endpoint_url("ftp://example.com").is_err());
    assert!(validate_endpoint_url("not-a-url").is_err());

    assert!(validate_bucket_name("my-bucket-01").is_ok());
    assert!(validate_bucket_name("ab").is_err());
    assert!(validate_bucket_name("MyBucket").is_err());
    assert!(validate_bucket_name("-bad-start").is_err());

    let client = S3Client::new();
    let acc = make_account("https://acct.r2.cloudflarestorage.com");
    let url = client
        .presign_url(&acc, "GET", "my-bucket", "dir/my file+1.txt", 900)
        .unwrap();
    assert!(url.starts_with("https://acct.r2.cloudflarestorage.com/my-bucket/dir/my%20file%2B1.txt?"));
    assert!(url.contains("X-Amz-Algorithm=AWS4-HMAC-SHA256"));
    assert!(url.contains("X-Amz-Expires=900"));
    assert!(url.contains("X-Amz-Signature="));
    assert!(!url.contains("SECRETEXAMPLEKEY"));
}

#[tokio::test]
async fn test_s3_client_bucket_and_object_crud() {
    let st = Arc::new(Mutex::new(MockS3State::default()));
    let endpoint = spawn_mock_s3(st.clone()).await;
    let client = S3Client::new();
    let acc = make_account(&endpoint);

    // Create & list buckets
    client.create_bucket(&acc, "alpha-bucket").await.unwrap();
    let buckets = client.list_buckets(&acc).await.unwrap();
    assert_eq!(buckets.len(), 1);
    assert_eq!(buckets[0].name, "alpha-bucket");
    assert!(st
        .lock()
        .unwrap()
        .last_auth_header
        .as_deref()
        .unwrap_or("")
        .starts_with("AWS4-HMAC-SHA256 Credential=AKIAEXAMPLE/"));

    // Put object with spaces & folder structure
    client
        .put_object(&acc, "alpha-bucket", "docs/hello world.txt", b"hello s3".to_vec(), "text/plain")
        .await
        .unwrap();
    client
        .put_object(&acc, "alpha-bucket", "root.txt", b"root-data".to_vec(), "text/plain")
        .await
        .unwrap();

    // List root with delimiter /
    let root_list = client
        .list_objects_v2(&acc, "alpha-bucket", "", None, 100)
        .await
        .unwrap();
    assert_eq!(root_list.entries.len(), 2);
    assert!(root_list.entries.iter().any(|e| e.is_dir && e.key == "docs/"));
    assert!(root_list.entries.iter().any(|e| !e.is_dir && e.key == "root.txt" && e.size == 9));

    // Get object stream
    let resp = client
        .get_object_stream(&acc, "alpha-bucket", "docs/hello world.txt")
        .await
        .unwrap();
    let bytes = resp.bytes().await.unwrap();
    assert_eq!(bytes.as_ref(), b"hello s3");

    // Copy across buckets
    client.create_bucket(&acc, "beta-bucket").await.unwrap();
    client
        .copy_object_across_buckets(&acc, "alpha-bucket", "root.txt", "beta-bucket", "copied.txt")
        .await
        .unwrap();
    let copied = client
        .get_object_stream(&acc, "beta-bucket", "copied.txt")
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert_eq!(copied.as_ref(), b"root-data");

    // Bulk parallel delete
    let del = client
        .delete_objects_parallel(
            &acc,
            "alpha-bucket",
            vec!["root.txt".into(), "docs/hello world.txt".into()],
        )
        .await;
    assert_eq!(del.deleted.len(), 2);
    assert!(del.failed.is_empty());

    // Delete bucket
    client.delete_bucket(&acc, "alpha-bucket").await.unwrap();
    let buckets_after = client.list_buckets(&acc).await.unwrap();
    assert_eq!(buckets_after.len(), 1);
    assert_eq!(buckets_after[0].name, "beta-bucket");
}

#[tokio::test]
async fn test_verify_account_falls_back_to_default_bucket_on_403() {
    let st = Arc::new(Mutex::new(MockS3State {
        forbid_list_buckets: true,
        ..Default::default()
    }));
    let endpoint = spawn_mock_s3(st).await;
    let client = S3Client::new();
    let acc = make_account(&endpoint);

    let res = client.verify_account(&acc).await.unwrap();
    assert!(res.ok);
    assert!(res.scoped_bucket_fallback);
    assert_eq!(res.buckets_count, 1);
}
