use crate::controllers::servers::{check_auth, AppState};
use crate::models::db::DbError;
use crate::models::storage::{
    DecryptedStorageAccount, StorageAccountInput, StorageAccountOutput, StorageAccountUpdateInput,
    StorageAccountVerifyInput, StorageBucket, StorageCopyIn, StorageCreateBucketIn,
    StorageCreateFolderIn, StorageDeleteIn, StorageDeleteOut, StorageListOut, StoragePresignIn,
    StoragePresignOut, StorageVerifyOut,
};
use crate::services::s3::{validate_bucket_name, validate_endpoint_url, S3Client, S3Error};
use crate::services::storage_transfer::{
    StorageTransferJob, StorageTransferManager, StorageTransferSpec,
};
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::Response,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Clone)]
pub struct StorageAppState {
    pub app: AppState,
    pub s3: S3Client,
    pub storage_transfers: StorageTransferManager,
}

#[derive(Deserialize)]
struct ListObjectsQuery {
    #[serde(default)]
    prefix: String,
    #[serde(default)]
    continuation_token: Option<String>,
    #[serde(default = "default_max_keys")]
    max_keys: u32,
}

fn default_max_keys() -> u32 {
    200
}

#[derive(Deserialize)]
struct DownloadQuery {
    key: String,
}

#[derive(Deserialize)]
struct CreateTransferIn {
    direction: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_true")]
    overwrite: bool,
    // SSH fields
    #[serde(default)]
    server_id: Option<i64>,
    #[serde(default)]
    ssh_path: Option<String>,
    // Single-S3 fields (for ssh_to_s3 / s3_to_ssh)
    #[serde(default)]
    account_id: Option<i64>,
    #[serde(default)]
    bucket: Option<String>,
    #[serde(default)]
    s3_prefix: Option<String>,
    // Dual-S3 fields (for s3_to_s3)
    #[serde(default)]
    source_account_id: Option<i64>,
    #[serde(default)]
    source_bucket: Option<String>,
    #[serde(default)]
    source_prefix: Option<String>,
    #[serde(default)]
    dest_account_id: Option<i64>,
    #[serde(default)]
    dest_bucket: Option<String>,
    #[serde(default)]
    dest_prefix: Option<String>,
}

fn default_mode() -> String {
    "auto".to_string()
}

fn default_true() -> bool {
    true
}

pub fn router(state: StorageAppState) -> Router {
    Router::new()
        .route("/storage/accounts", get(list_accounts).post(create_account))
        .route("/storage/accounts/verify", post(verify_account))
        .route(
            "/storage/accounts/:id",
            put(update_account).delete(delete_account),
        )
        .route(
            "/storage/accounts/:id/buckets",
            get(list_buckets).post(create_bucket),
        )
        .route(
            "/storage/accounts/:id/buckets/:bucket",
            delete(delete_bucket),
        )
        .route(
            "/storage/accounts/:id/buckets/:bucket/objects",
            get(list_objects),
        )
        .route(
            "/storage/accounts/:id/buckets/:bucket/folder",
            post(create_folder),
        )
        .route(
            "/storage/accounts/:id/buckets/:bucket/upload",
            post(upload_objects).layer(DefaultBodyLimit::max(512 * 1024 * 1024)),
        )
        .route(
            "/storage/accounts/:id/buckets/:bucket/download",
            get(download_object),
        )
        .route(
            "/storage/accounts/:id/buckets/:bucket/copy",
            post(copy_object),
        )
        .route(
            "/storage/accounts/:id/buckets/:bucket/delete",
            post(delete_objects),
        )
        .route(
            "/storage/accounts/:id/buckets/:bucket/presign",
            post(presign_object),
        )
        .route(
            "/storage/transfers",
            get(list_transfers).post(create_transfer),
        )
        .route("/storage/transfers/:job_id", get(get_transfer))
        .route("/storage/transfers/:job_id/cancel", post(cancel_transfer))
        .with_state(state)
}

fn map_s3_err(e: S3Error) -> (StatusCode, String) {
    match e {
        S3Error::InvalidEndpoint(m) | S3Error::InvalidBucket(m) | S3Error::InvalidKey(m) => {
            (StatusCode::BAD_REQUEST, m)
        }
        S3Error::Remote { status, message, .. } => {
            let st = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
            (st, message)
        }
        S3Error::Http(m) | S3Error::Xml(m) => (StatusCode::BAD_GATEWAY, m),
    }
}

fn map_db_err(e: DbError) -> (StatusCode, String) {
    match e {
        DbError::NotFound => (StatusCode::NOT_FOUND, "Storage account tidak ditemukan".into()),
        other => (StatusCode::INTERNAL_SERVER_ERROR, other.to_string()),
    }
}

async fn get_kek(state: &StorageAppState) -> Result<zeroize::Zeroizing<[u8; 32]>, (StatusCode, String)> {
    state
        .app
        .vault
        .get_kek()
        .await
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Vault terkunci".into()))
}

async fn list_accounts(
    State(state): State<StorageAppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<StorageAccountOutput>>, (StatusCode, String)> {
    check_auth(&headers, &state.app.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;
    state
        .app
        .db
        .list_storage_accounts()
        .map(Json)
        .map_err(map_db_err)
}

async fn create_account(
    State(state): State<StorageAppState>,
    headers: HeaderMap,
    Json(input): Json<StorageAccountInput>,
) -> Result<Json<StorageAccountOutput>, (StatusCode, String)> {
    check_auth(&headers, &state.app.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;
    if input.name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Nama akun wajib diisi".into()));
    }
    if input.access_key_id.trim().is_empty() || input.secret_access_key.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Access Key ID dan Secret Access Key wajib diisi".into(),
        ));
    }
    validate_endpoint_url(&input.endpoint).map_err(map_s3_err)?;
    let kek = get_kek(&state).await?;
    let id = state
        .app
        .db
        .create_storage_account(&input, &kek)
        .map_err(map_db_err)?;
    state
        .app
        .db
        .get_storage_account(id)
        .map(Json)
        .map_err(map_db_err)
}

async fn update_account(
    State(state): State<StorageAppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<StorageAccountUpdateInput>,
) -> Result<Json<StorageAccountOutput>, (StatusCode, String)> {
    check_auth(&headers, &state.app.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;
    if input.name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Nama akun wajib diisi".into()));
    }
    validate_endpoint_url(&input.endpoint).map_err(map_s3_err)?;
    let kek = get_kek(&state).await?;
    state
        .app
        .db
        .update_storage_account(id, &input, &kek)
        .map_err(map_db_err)?;
    state
        .app
        .db
        .get_storage_account(id)
        .map(Json)
        .map_err(map_db_err)
}

async fn delete_account(
    State(state): State<StorageAppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Value>, (StatusCode, String)> {
    check_auth(&headers, &state.app.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;
    state
        .app
        .db
        .delete_storage_account(id)
        .map_err(map_db_err)?;
    Ok(Json(json!({ "ok": true })))
}

async fn verify_account(
    State(state): State<StorageAppState>,
    headers: HeaderMap,
    Json(input): Json<StorageAccountVerifyInput>,
) -> Result<Json<StorageVerifyOut>, (StatusCode, String)> {
    check_auth(&headers, &state.app.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;
    validate_endpoint_url(&input.endpoint).map_err(map_s3_err)?;
    let secret = match &input.secret_access_key {
        Some(s) if !s.is_empty() => s.clone(),
        _ => {
            if let Some(acc_id) = input.account_id {
                let kek = get_kek(&state).await?;
                let existing = state
                    .app
                    .db
                    .get_storage_account_decrypted(acc_id, &kek)
                    .map_err(map_db_err)?;
                existing.secret_access_key
            } else {
                return Err((StatusCode::BAD_REQUEST, "Secret Access Key wajib diisi".into()));
            }
        }
    };
    let tmp_acc = DecryptedStorageAccount {
        id: input.account_id.unwrap_or(0),
        name: "verify".into(),
        provider: input.provider,
        endpoint: input.endpoint,
        region: input.region,
        access_key_id: input.access_key_id,
        secret_access_key: secret,
        default_bucket: input.default_bucket,
        path_style: input.path_style,
    };
    let out = state
        .s3
        .verify_account(&tmp_acc)
        .await
        .map_err(map_s3_err)?;
    Ok(Json(out))
}

async fn list_buckets(
    State(state): State<StorageAppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Vec<StorageBucket>>, (StatusCode, String)> {
    check_auth(&headers, &state.app.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;
    let kek = get_kek(&state).await?;
    let acc = state
        .app
        .db
        .get_storage_account_decrypted(id, &kek)
        .map_err(map_db_err)?;
    match state.s3.list_buckets(&acc).await {
        Ok(buckets) => Ok(Json(buckets)),
        Err(S3Error::Remote { status: 403, .. }) if !acc.default_bucket.trim().is_empty() => {
            Ok(Json(vec![StorageBucket {
                name: acc.default_bucket.trim().to_string(),
                creation_date: String::new(),
            }]))
        }
        Err(e) => Err(map_s3_err(e)),
    }
}

async fn create_bucket(
    State(state): State<StorageAppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<StorageCreateBucketIn>,
) -> Result<Json<Value>, (StatusCode, String)> {
    check_auth(&headers, &state.app.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;
    validate_bucket_name(&input.name).map_err(map_s3_err)?;
    let kek = get_kek(&state).await?;
    let acc = state
        .app
        .db
        .get_storage_account_decrypted(id, &kek)
        .map_err(map_db_err)?;
    state
        .s3
        .create_bucket(&acc, input.name.trim())
        .await
        .map_err(map_s3_err)?;
    Ok(Json(json!({ "ok": true, "name": input.name.trim() })))
}

async fn delete_bucket(
    State(state): State<StorageAppState>,
    headers: HeaderMap,
    Path((id, bucket)): Path<(i64, String)>,
) -> Result<Json<Value>, (StatusCode, String)> {
    check_auth(&headers, &state.app.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;
    let kek = get_kek(&state).await?;
    let acc = state
        .app
        .db
        .get_storage_account_decrypted(id, &kek)
        .map_err(map_db_err)?;
    state
        .s3
        .delete_bucket(&acc, &bucket)
        .await
        .map_err(map_s3_err)?;
    Ok(Json(json!({ "ok": true })))
}

async fn list_objects(
    State(state): State<StorageAppState>,
    headers: HeaderMap,
    Path((id, bucket)): Path<(i64, String)>,
    Query(q): Query<ListObjectsQuery>,
) -> Result<Json<StorageListOut>, (StatusCode, String)> {
    check_auth(&headers, &state.app.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;
    let kek = get_kek(&state).await?;
    let acc = state
        .app
        .db
        .get_storage_account_decrypted(id, &kek)
        .map_err(map_db_err)?;
    let out = state
        .s3
        .list_objects_v2(
            &acc,
            &bucket,
            &q.prefix,
            q.continuation_token.as_deref(),
            q.max_keys,
        )
        .await
        .map_err(map_s3_err)?;
    Ok(Json(out))
}

async fn create_folder(
    State(state): State<StorageAppState>,
    headers: HeaderMap,
    Path((id, bucket)): Path<(i64, String)>,
    Json(input): Json<StorageCreateFolderIn>,
) -> Result<Json<Value>, (StatusCode, String)> {
    check_auth(&headers, &state.app.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;
    let clean = input.prefix.trim().trim_start_matches('/');
    if clean.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Nama folder wajib diisi".into()));
    }
    let key = if clean.ends_with('/') {
        clean.to_string()
    } else {
        format!("{}/", clean)
    };
    let kek = get_kek(&state).await?;
    let acc = state
        .app
        .db
        .get_storage_account_decrypted(id, &kek)
        .map_err(map_db_err)?;
    state
        .s3
        .put_object(&acc, &bucket, &key, Vec::new(), "application/x-directory")
        .await
        .map_err(map_s3_err)?;
    Ok(Json(json!({ "ok": true, "key": key })))
}

async fn upload_objects(
    State(state): State<StorageAppState>,
    headers: HeaderMap,
    Path((id, bucket)): Path<(i64, String)>,
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, String)> {
    check_auth(&headers, &state.app.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;
    let kek = get_kek(&state).await?;
    let acc = state
        .app
        .db
        .get_storage_account_decrypted(id, &kek)
        .map_err(map_db_err)?;

    let mut prefix = String::new();
    let mut uploaded: Vec<String> = Vec::new();
    let mut pending_files: Vec<(String, String, Vec<u8>)> = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "prefix" {
            prefix = field
                .text()
                .await
                .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
            continue;
        }
        let filename = field
            .file_name()
            .unwrap_or("upload.bin")
            .trim_start_matches('/')
            .to_string();
        let ct = field
            .content_type()
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                mime_guess::from_path(&filename)
                    .first_or_octet_stream()
                    .essence_str()
                    .to_string()
            });
        let data = field
            .bytes()
            .await
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
            .to_vec();
        pending_files.push((filename, ct, data));
    }

    let p = prefix.trim().trim_start_matches('/');
    for (filename, ct, data) in pending_files {
        let key = if p.is_empty() {
            filename
        } else if p.ends_with('/') {
            format!("{}{}", p, filename)
        } else {
            format!("{}/{}", p, filename)
        };
        state
            .s3
            .put_object(&acc, &bucket, &key, data, &ct)
            .await
            .map_err(map_s3_err)?;
        uploaded.push(key);
    }

    Ok(Json(json!({ "ok": true, "uploaded": uploaded })))
}

async fn download_object(
    State(state): State<StorageAppState>,
    headers: HeaderMap,
    Path((id, bucket)): Path<(i64, String)>,
    Query(q): Query<DownloadQuery>,
) -> Result<Response, (StatusCode, String)> {
    check_auth(&headers, &state.app.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;
    let kek = get_kek(&state).await?;
    let acc = state
        .app
        .db
        .get_storage_account_decrypted(id, &kek)
        .map_err(map_db_err)?;
    let resp = state
        .s3
        .get_object_stream(&acc, &bucket, &q.key)
        .await
        .map_err(map_s3_err)?;

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    let filename = q
        .key
        .rsplit('/')
        .next()
        .unwrap_or("download.bin")
        .replace(['"', '\\', '\r', '\n'], "_");

    let stream = resp.bytes_stream();
    let body = Body::from_stream(stream);
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type);
    if let Ok(cd) = HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename)) {
        builder = builder.header(header::CONTENT_DISPOSITION, cd);
    }
    builder
        .body(body)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

async fn copy_object(
    State(state): State<StorageAppState>,
    headers: HeaderMap,
    Path((id, bucket)): Path<(i64, String)>,
    Json(input): Json<StorageCopyIn>,
) -> Result<Json<Value>, (StatusCode, String)> {
    check_auth(&headers, &state.app.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;
    let kek = get_kek(&state).await?;
    let acc = state
        .app
        .db
        .get_storage_account_decrypted(id, &kek)
        .map_err(map_db_err)?;
    state
        .s3
        .copy_object_across_buckets(&acc, &bucket, &input.source_key, &bucket, &input.dest_key)
        .await
        .map_err(map_s3_err)?;
    if input.delete_source {
        state
            .s3
            .delete_object(&acc, &bucket, &input.source_key)
            .await
            .map_err(map_s3_err)?;
    }
    Ok(Json(json!({ "ok": true, "dest_key": input.dest_key })))
}

async fn delete_objects(
    State(state): State<StorageAppState>,
    headers: HeaderMap,
    Path((id, bucket)): Path<(i64, String)>,
    Json(input): Json<StorageDeleteIn>,
) -> Result<Json<StorageDeleteOut>, (StatusCode, String)> {
    check_auth(&headers, &state.app.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;
    let kek = get_kek(&state).await?;
    let acc = state
        .app
        .db
        .get_storage_account_decrypted(id, &kek)
        .map_err(map_db_err)?;
    let out = state
        .s3
        .delete_objects_parallel(&acc, &bucket, input.keys)
        .await;
    Ok(Json(out))
}

async fn presign_object(
    State(state): State<StorageAppState>,
    headers: HeaderMap,
    Path((id, bucket)): Path<(i64, String)>,
    Json(input): Json<StoragePresignIn>,
) -> Result<Json<StoragePresignOut>, (StatusCode, String)> {
    check_auth(&headers, &state.app.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;
    let kek = get_kek(&state).await?;
    let acc = state
        .app
        .db
        .get_storage_account_decrypted(id, &kek)
        .map_err(map_db_err)?;
    let expires = input.expires_secs.clamp(60, 7 * 24 * 3600);
    let url = state
        .s3
        .presign_url(&acc, "GET", &bucket, &input.key, expires)
        .map_err(map_s3_err)?;
    Ok(Json(StoragePresignOut {
        url,
        expires_secs: expires,
    }))
}

async fn list_transfers(
    State(state): State<StorageAppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<StorageTransferJob>>, (StatusCode, String)> {
    check_auth(&headers, &state.app.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;
    Ok(Json(state.storage_transfers.list_jobs()))
}

async fn get_transfer(
    State(state): State<StorageAppState>,
    headers: HeaderMap,
    Path(job_id): Path<String>,
) -> Result<Json<StorageTransferJob>, (StatusCode, String)> {
    check_auth(&headers, &state.app.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;
    state
        .storage_transfers
        .get_job(&job_id)
        .map(Json)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Transfer job tidak ditemukan".into()))
}

async fn cancel_transfer(
    State(state): State<StorageAppState>,
    headers: HeaderMap,
    Path(job_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    check_auth(&headers, &state.app.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;
    if state.storage_transfers.cancel_job(&job_id) {
        Ok(Json(json!({ "ok": true })))
    } else {
        Err((StatusCode::NOT_FOUND, "Transfer job tidak ditemukan".into()))
    }
}

async fn create_transfer(
    State(state): State<StorageAppState>,
    headers: HeaderMap,
    Json(input): Json<CreateTransferIn>,
) -> Result<Json<StorageTransferJob>, (StatusCode, String)> {
    check_auth(&headers, &state.app.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;
    let kek = get_kek(&state).await?;
    let spec = StorageTransferSpec {
        direction: input.direction.clone(),
        mode: input.mode.clone(),
        overwrite: input.overwrite,
    };

    match input.direction.as_str() {
        "ssh_to_s3" => {
            let server_id = input
                .server_id
                .ok_or_else(|| (StatusCode::BAD_REQUEST, "server_id wajib diisi".into()))?;
            let ssh_path = input
                .ssh_path
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| (StatusCode::BAD_REQUEST, "ssh_path wajib diisi".into()))?;
            let account_id = input
                .account_id
                .ok_or_else(|| (StatusCode::BAD_REQUEST, "account_id wajib diisi".into()))?;
            let bucket = input
                .bucket
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| (StatusCode::BAD_REQUEST, "bucket wajib diisi".into()))?;
            let s3_prefix = input.s3_prefix.unwrap_or_default();

            let creds = state
                .app
                .db
                .get_server_credentials(server_id, &kek)
                .map_err(map_db_err)?;
            let acc = state
                .app
                .db
                .get_storage_account_decrypted(account_id, &kek)
                .map_err(map_db_err)?;

            let job = state.storage_transfers.start_ssh_to_s3(
                state.s3.clone(),
                state.app.pool.clone(),
                spec,
                creds,
                ssh_path,
                acc,
                bucket,
                s3_prefix,
            );
            Ok(Json(job))
        }
        "s3_to_ssh" => {
            let server_id = input
                .server_id
                .ok_or_else(|| (StatusCode::BAD_REQUEST, "server_id wajib diisi".into()))?;
            let ssh_path = input
                .ssh_path
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| (StatusCode::BAD_REQUEST, "ssh_path wajib diisi".into()))?;
            let account_id = input
                .account_id
                .ok_or_else(|| (StatusCode::BAD_REQUEST, "account_id wajib diisi".into()))?;
            let bucket = input
                .bucket
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| (StatusCode::BAD_REQUEST, "bucket wajib diisi".into()))?;
            let s3_prefix = input.s3_prefix.unwrap_or_default();

            let creds = state
                .app
                .db
                .get_server_credentials(server_id, &kek)
                .map_err(map_db_err)?;
            let acc = state
                .app
                .db
                .get_storage_account_decrypted(account_id, &kek)
                .map_err(map_db_err)?;

            let job = state.storage_transfers.start_s3_to_ssh(
                state.s3.clone(),
                state.app.pool.clone(),
                spec,
                acc,
                bucket,
                s3_prefix,
                creds,
                ssh_path,
            );
            Ok(Json(job))
        }
        "s3_to_s3" => {
            let src_id = input
                .source_account_id
                .or(input.account_id)
                .ok_or_else(|| (StatusCode::BAD_REQUEST, "source_account_id wajib diisi".into()))?;
            let src_bucket = input
                .source_bucket
                .or(input.bucket)
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| (StatusCode::BAD_REQUEST, "source_bucket wajib diisi".into()))?;
            let src_prefix = input.source_prefix.or(input.s3_prefix).unwrap_or_default();

            let dst_id = input
                .dest_account_id
                .ok_or_else(|| (StatusCode::BAD_REQUEST, "dest_account_id wajib diisi".into()))?;
            let dst_bucket = input
                .dest_bucket
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| (StatusCode::BAD_REQUEST, "dest_bucket wajib diisi".into()))?;
            let dst_prefix = input.dest_prefix.unwrap_or_default();

            let src_acc = state
                .app
                .db
                .get_storage_account_decrypted(src_id, &kek)
                .map_err(map_db_err)?;
            let dst_acc = state
                .app
                .db
                .get_storage_account_decrypted(dst_id, &kek)
                .map_err(map_db_err)?;

            let job = state.storage_transfers.start_s3_to_s3(
                state.s3.clone(),
                spec,
                src_acc,
                src_bucket,
                src_prefix,
                dst_acc,
                dst_bucket,
                dst_prefix,
            );
            Ok(Json(job))
        }
        other => Err((
            StatusCode::BAD_REQUEST,
            format!("Direction tidak dikenali: '{}'", other),
        )),
    }
}
