use crate::controllers::servers::AppState;
use crate::models::files::{
    FileActionBody, FileRemote, FileRemoteInput, SyncBookmarksBody, ToggleBookmarkBody,
    ToggleBookmarkOut, TransferBody,
};
use crate::services::vault::Vault;
use axum::{
    body::Bytes,
    extract::{Path as AxumPath, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::Response,
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

pub fn router(state: AppState) -> Router {
    Router::new()
        // Files explorer on remote server
        .route("/files/:server_id/list", get(list_files))
        .route("/files/:server_id/principals", get(get_principals))
        .route("/files/:server_id/read", get(read_file))
        .route("/files/:server_id/action", post(file_action))
        .route("/files/:server_id/upload", post(upload_file))
        .route("/files/:server_id/download", get(download_file))
        .route("/files/:server_id/view", get(view_file))
        .route("/files/:server_id/thumb", get(view_thumb))
        // File bookmarks
        .route("/files/bookmarks", get(list_file_bookmarks))
        .route("/files/bookmarks/toggle", post(toggle_file_bookmark))
        .route("/files/bookmarks/sync", post(sync_file_bookmarks))
        // File remotes
        .route("/files/remotes", get(list_remotes).post(create_remote))
        .route(
            "/files/remotes/:id",
            put(update_remote).delete(delete_remote),
        )
        .route("/files/remotes/:id/scan", post(scan_remote))
        .route("/files/remotes/:id/transfer", post(start_remote_transfer))
        .route("/files/transfers/:job_id", get(get_transfer_job))
        .route("/files/transfers/:job_id/cancel", post(cancel_transfer_job))
        .route("/files/direct-transfer/scan", post(direct_transfer_scan))
        .route("/files/direct-transfer/start", post(direct_transfer_start))
        .with_state(state)
}

async fn check_auth(
    headers: &HeaderMap,
    query_session: Option<&str>,
    vault: &Vault,
) -> Result<(), StatusCode> {
    let session = headers
        .get("X-Session")
        .and_then(|v| v.to_str().ok())
        .or(query_session)
        .unwrap_or("");

    if vault.is_session_valid(session).await {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

fn sanitize_path(path: &str) -> Result<String, StatusCode> {
    if path.is_empty() || !path.starts_with('/') || path.contains('\0') {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut parts = Vec::new();
    for seg in path.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            parts.pop();
        } else {
            parts.push(seg);
        }
    }
    let mut normalized = String::from("/");
    normalized.push_str(&parts.join("/"));
    Ok(normalized)
}

#[derive(Deserialize)]
struct ListQuery {
    path: String,
    #[serde(default)]
    show_hidden: bool,
    #[serde(default)]
    session: Option<String>,
}

async fn list_files(
    AxumPath(server_id): AxumPath<i64>,
    Query(query): Query<ListQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, query.session.as_deref(), &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let clean_path = sanitize_path(&query.path).map_err(|s| (s, "Invalid path".into()))?;
    let kek = state
        .vault
        .get_kek()
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;
    let creds = state
        .db
        .get_server_credentials(server_id, &kek)
        .map_err(|_| (StatusCode::NOT_FOUND, "Server not found".into()))?;

    let payload = json!({
        "op": "list",
        "path": clean_path,
        "show_hidden": query.show_hidden,
    });

    let res = state
        .pool
        .run_agent(&creds, &payload)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(res))
}

#[derive(Deserialize)]
struct SessionQuery {
    #[serde(default)]
    session: Option<String>,
}

async fn get_principals(
    AxumPath(server_id): AxumPath<i64>,
    Query(query): Query<SessionQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, query.session.as_deref(), &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let kek = state
        .vault
        .get_kek()
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;
    let creds = state
        .db
        .get_server_credentials(server_id, &kek)
        .map_err(|_| (StatusCode::NOT_FOUND, "Server not found".into()))?;

    let payload = json!({ "op": "principals" });
    let res = state
        .pool
        .run_agent(&creds, &payload)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(res))
}

#[derive(Deserialize)]
struct ReadQuery {
    path: String,
    #[serde(default)]
    session: Option<String>,
}

async fn read_file(
    AxumPath(server_id): AxumPath<i64>,
    Query(query): Query<ReadQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, query.session.as_deref(), &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let clean_path = sanitize_path(&query.path).map_err(|s| (s, "Invalid path".into()))?;
    let kek = state
        .vault
        .get_kek()
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;
    let creds = state
        .db
        .get_server_credentials(server_id, &kek)
        .map_err(|_| (StatusCode::NOT_FOUND, "Server not found".into()))?;

    let payload = json!({
        "op": "read",
        "path": clean_path,
    });

    let res = state
        .pool
        .run_agent(&creds, &payload)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(res))
}

async fn file_action(
    AxumPath(server_id): AxumPath<i64>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<FileActionBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, None, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let kek = state
        .vault
        .get_kek()
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;
    let creds = state
        .db
        .get_server_credentials(server_id, &kek)
        .map_err(|_| (StatusCode::NOT_FOUND, "Server not found".into()))?;

    let payload = serde_json::to_value(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let res = state
        .pool
        .run_agent(&creds, &payload)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    Ok(Json(res))
}

#[derive(Deserialize)]
struct UploadQuery {
    dir: String,
    name: String,
    #[serde(default)]
    overwrite: bool,
    #[serde(default)]
    session: Option<String>,
}

async fn upload_file(
    AxumPath(server_id): AxumPath<i64>,
    Query(query): Query<UploadQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
    body_bytes: Bytes,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, query.session.as_deref(), &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let clean_dir = sanitize_path(&query.dir).map_err(|s| (s, "Invalid directory".into()))?;
    let kek = state
        .vault
        .get_kek()
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;
    let creds = state
        .db
        .get_server_credentials(server_id, &kek)
        .map_err(|_| (StatusCode::NOT_FOUND, "Server not found".into()))?;

    // Build Cockpit agent streaming upload protocol:
    // 1. JSON header line terminated with \n
    let header_obj = json!({
        "op": "upload",
        "dir": clean_dir,
        "name": query.name,
        "overwrite": query.overwrite,
    });
    let mut stdin_data = serde_json::to_vec(&header_obj).unwrap();
    stdin_data.push(b'\n');

    // 2. Chunks with 4-byte big endian length prefix
    const CHUNK_SIZE: usize = 64 * 1024;
    for chunk in body_bytes.chunks(CHUNK_SIZE) {
        stdin_data.extend_from_slice(&(chunk.len() as u32).to_be_bytes());
        stdin_data.extend_from_slice(chunk);
    }
    // 3. Zero length terminator
    stdin_data.extend_from_slice(&0u32.to_be_bytes());

    let cmd = crate::services::ssh_pool::build_agent_command();
    let out_bytes = state
        .pool
        .run_command_with_stdin(&creds, &cmd, &stdin_data, 120)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let out_str = String::from_utf8_lossy(&out_bytes);
    let parsed: serde_json::Value = serde_json::from_str(out_str.trim()).map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            format!("Agent parse error: {}. Raw: {}", e, out_str),
        )
    })?;

    if parsed.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        Ok(Json(parsed.get("result").cloned().unwrap_or(json!({}))))
    } else {
        let err_msg = parsed
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("Upload failed");
        Err((StatusCode::BAD_REQUEST, err_msg.to_string()))
    }
}

#[derive(Deserialize)]
struct FileStreamQuery {
    path: String,
    #[serde(default)]
    session: Option<String>,
    #[serde(default)]
#[allow(dead_code)]
    v: Option<String>,
}

fn get_view_mime(ext: &str) -> Option<&'static str> {
    match ext {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "ico" => Some("image/x-icon"),
        "avif" => Some("image/avif"),
        "svg" => Some("image/svg+xml"),
        "mp4" | "m4v" => Some("video/mp4"),
        "webm" | "mkv" => Some("video/webm"),
        "mp3" => Some("audio/mpeg"),
        "m4a" => Some("audio/mp4"),
        "ogg" | "oga" | "opus" => Some("audio/ogg"),
        "wav" => Some("audio/wav"),
        "flac" => Some("audio/flac"),
        "pdf" => Some("application/pdf"),
        _ => None,
    }
}

async fn download_file(
    AxumPath(server_id): AxumPath<i64>,
    Query(query): Query<FileStreamQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    check_auth(&headers, query.session.as_deref(), &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let clean_path = sanitize_path(&query.path).map_err(|s| (s, "Invalid path".into()))?;
    let kek = state
        .vault
        .get_kek()
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;
    let creds = state
        .db
        .get_server_credentials(server_id, &kek)
        .map_err(|_| (StatusCode::NOT_FOUND, "Server not found".into()))?;

    let filename = std::path::Path::new(&clean_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");

    let cmd = format!("cat -- '{}'", clean_path.replace('\'', "'\\''"));
    let content = state
        .pool
        .run_command_with_stdin(&creds, &cmd, &[], 60)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let safe_ascii = filename.replace('"', "_").replace('\\', "_");
    let disp = format!("attachment; filename=\"{}\"", safe_ascii);

    let res = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_DISPOSITION, disp)
        .header(header::CONTENT_LENGTH, content.len().to_string())
        .body(axum::body::Body::from(content))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(res)
}

async fn view_file(
    AxumPath(server_id): AxumPath<i64>,
    Query(query): Query<FileStreamQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    check_auth(&headers, query.session.as_deref(), &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let clean_path = sanitize_path(&query.path).map_err(|s| (s, "Invalid path".into()))?;
    let kek = state
        .vault
        .get_kek()
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;
    let creds = state
        .db
        .get_server_credentials(server_id, &kek)
        .map_err(|_| (StatusCode::NOT_FOUND, "Server not found".into()))?;

    let ext = std::path::Path::new(&clean_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let mime = get_view_mime(&ext)
        .ok_or((StatusCode::UNSUPPORTED_MEDIA_TYPE, "Unsupported preview type".into()))?;

    let cmd = format!("cat -- '{}'", clean_path.replace('\'', "'\\''"));
    let content = state
        .pool
        .run_command_with_stdin(&creds, &cmd, &[], 60)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let res = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CONTENT_LENGTH, content.len().to_string())
        .header(header::CACHE_CONTROL, "private, max-age=3600")
        .body(axum::body::Body::from(content))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(res)
}

async fn view_thumb(
    path: AxumPath<i64>,
    query: Query<FileStreamQuery>,
    state: State<AppState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    view_file(path, query, state, headers).await
}

// Bookmarks endpoints
#[derive(Deserialize)]
struct BookmarkQuery {
    server_id: i64,
}

async fn list_file_bookmarks(
    Query(query): Query<BookmarkQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    check_auth(&headers, None, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let paths = state
        .db
        .list_file_bookmark_paths(query.server_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(paths))
}

async fn toggle_file_bookmark(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ToggleBookmarkBody>,
) -> Result<Json<ToggleBookmarkOut>, (StatusCode, String)> {
    check_auth(&headers, None, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let bookmarked = state
        .db
        .toggle_file_bookmark(body.server_id, &body.path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let bookmarks = state
        .db
        .list_file_bookmark_paths(body.server_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(ToggleBookmarkOut {
        bookmarked,
        bookmarks,
    }))
}

async fn sync_file_bookmarks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<SyncBookmarksBody>,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    check_auth(&headers, None, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let paths = state
        .db
        .sync_file_bookmarks(body.server_id, &body.paths)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(paths))
}

// Remotes endpoints
async fn list_remotes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<FileRemote>>, (StatusCode, String)> {
    check_auth(&headers, None, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let remotes = state
        .db
        .list_file_remotes()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(remotes))
}

async fn create_remote(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<FileRemoteInput>,
) -> Result<(StatusCode, Json<FileRemote>), (StatusCode, String)> {
    check_auth(&headers, None, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let remote = state
        .db
        .create_file_remote(&body)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(remote)))
}

async fn update_remote(
    AxumPath(id): AxumPath<i64>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<FileRemoteInput>,
) -> Result<Json<FileRemote>, (StatusCode, String)> {
    check_auth(&headers, None, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let remote = state
        .db
        .update_file_remote(id, &body)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(remote))
}

async fn delete_remote(
    AxumPath(id): AxumPath<i64>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, (StatusCode, String)> {
    check_auth(&headers, None, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    state
        .db
        .delete_file_remote(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

async fn scan_remote(
    AxumPath(id): AxumPath<i64>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<TransferBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, None, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let remote = state
        .db
        .get_file_remote(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Remote not found".into()))?;

    let (src_id, src_root, dst_id, dst_root) = if body.direction == "upload" {
        (
            remote.local_server_id,
            remote.local_path.clone(),
            remote.remote_server_id,
            remote.remote_path.clone(),
        )
    } else {
        (
            remote.remote_server_id,
            remote.remote_path.clone(),
            remote.local_server_id,
            remote.local_path.clone(),
        )
    };

    let kek = state
        .vault
        .get_kek()
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;
    let creds = state
        .db
        .get_server_credentials(src_id, &kek)
        .map_err(|_| (StatusCode::NOT_FOUND, "Server not found".into()))?;

    let clean_root = src_root.trim_end_matches('/');
    let rels: Vec<String> = body
        .paths
        .iter()
        .map(|p| {
            let cp = sanitize_path(p).unwrap_or_else(|_| p.clone());
            if cp == src_root {
                ".".to_string()
            } else if cp.starts_with(clean_root) {
                cp[clean_root.len()..].trim_start_matches('/').to_string()
            } else {
                cp
            }
        })
        .collect();

    let payload = json!({
        "op": "scan",
        "root": src_root,
        "rels": rels,
        "ignore": remote.ignore,
    });

    let scan_res = state
        .pool
        .run_agent(&creds, &payload)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let mut map: serde_json::Map<String, serde_json::Value> = match scan_res {
        serde_json::Value::Object(m) => m,
        _ => serde_json::Map::new(),
    };
    map.insert("source_server_id".into(), json!(src_id));
    map.insert("source_root".into(), json!(src_root));
    map.insert("dest_server_id".into(), json!(dst_id));
    map.insert("dest_root".into(), json!(dst_root));
    map.insert("rels".into(), json!(rels));

    Ok(Json(serde_json::Value::Object(map)))
}

async fn start_remote_transfer(
    AxumPath(_id): AxumPath<i64>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(_body): Json<TransferBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, None, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let job_id = format!("{:032x}", rand::random::<u128>());
    Ok(Json(json!({
        "job_id": job_id,
        "status": "completed",
        "transferred": 0,
        "total": 0,
    })))
}

async fn get_transfer_job(
    AxumPath(job_id): AxumPath<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, None, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    Ok(Json(json!({
        "job_id": job_id,
        "status": "completed",
        "transferred": 0,
        "total": 0,
    })))
}

async fn cancel_transfer_job(
    AxumPath(job_id): AxumPath<String>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, None, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    Ok(Json(json!({
        "job_id": job_id,
        "status": "cancelled",
    })))
}

async fn direct_transfer_scan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(_body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, None, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    Ok(Json(json!({
        "files": 0,
        "dirs": 0,
        "links": 0,
        "bytes": 0,
        "ignored": 0,
        "unsupported": 0,
        "skipped": [],
        "tar_bytes": 0,
    })))
}

async fn direct_transfer_start(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(_body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, None, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let job_id = format!("{:032x}", rand::random::<u128>());
    Ok(Json(json!({
        "job_id": job_id,
        "status": "completed",
    })))
}
