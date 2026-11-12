use crate::models::db::DbStore;
use crate::models::server::{
    ConnectionCheck, DecryptedServerCreds, ServerInput, ServerOutput, ServerStats,
    TestServerInput,
};
use crate::services::cloudflare::CloudflareClient;
use crate::services::tailscale::TailscaleClient;
use crate::services::paste_image::{
    build_paste_request, paste_image_name, sniff_image_type, PASTE_MAX_BYTES,
};
use crate::services::ssh_pool::{build_agent_command, parse_stats, SshPool, STATS_SCRIPT};
use crate::services::vault::Vault;
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Clone)]
pub struct AppState {
    pub db: DbStore,
    pub vault: Vault,
    pub pool: SshPool,
    pub cf: CloudflareClient,
    pub ts: TailscaleClient,
    pub transfers: crate::services::transfer::TransferManager,
    pub updater: crate::services::updater::UpdateProgress,
}

#[derive(Deserialize)]
pub struct OrderInput {
    pub ids: Vec<i64>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/servers", get(list_servers).post(create_server))
        .route("/servers/order", put(reorder_servers).post(reorder_servers))
        .route("/servers/test", post(test_server))
        .route(
            "/servers/:id",
            get(get_server).put(update_server).delete(delete_server),
        )
        .route("/servers/:id/stats", get(server_stats))
        .route("/servers/:id/reset-hostkey", post(reset_host_key))
        .route("/servers/:id/ls", get(list_dir))
        .route(
            "/servers/:id/paste-image",
            // Room above the limit so oversized images reach the handler and get a JSON 413.
            post(paste_image).layer(DefaultBodyLimit::max(PASTE_MAX_BYTES + 64 * 1024)),
        )
        .with_state(state)
}

pub async fn check_auth(headers: &HeaderMap, vault: &Vault) -> Result<(), StatusCode> {
    let session = headers
        .get("X-Session")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if vault.is_session_valid(session).await {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn list_servers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ServerOutput>>, StatusCode> {
    check_auth(&headers, &state.vault).await?;
    state
        .db
        .list_servers()
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn create_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ServerInput>,
) -> Result<Json<ServerOutput>, StatusCode> {
    check_auth(&headers, &state.vault).await?;
    let kek = state.vault.get_kek().await.map_err(|_| StatusCode::UNAUTHORIZED)?;
    let id = state
        .db
        .create_server(&input, &kek)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let servers = state.db.list_servers().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let server = servers.into_iter().find(|s| s.id == id).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(server))
}

async fn get_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ServerOutput>, StatusCode> {
    check_auth(&headers, &state.vault).await?;
    let servers = state.db.list_servers().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let server = servers.into_iter().find(|s| s.id == id).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(server))
}

async fn update_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(input): Json<ServerInput>,
) -> Result<Json<ServerOutput>, StatusCode> {
    check_auth(&headers, &state.vault).await?;
    let kek = state.vault.get_kek().await.map_err(|_| StatusCode::UNAUTHORIZED)?;
    state.pool.disconnect(id).await;
    state
        .db
        .update_server(id, &input, &kek)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let servers = state.db.list_servers().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let server = servers.into_iter().find(|s| s.id == id).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(server))
}

async fn delete_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    check_auth(&headers, &state.vault).await?;
    state.pool.disconnect(id).await;
    state
        .db
        .delete_server(id)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn reorder_servers(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<OrderInput>,
) -> Result<Json<Vec<ServerOutput>>, StatusCode> {
    check_auth(&headers, &state.vault).await?;
    state
        .db
        .reorder_servers(&input.ids)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let servers = state.db.list_servers().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(servers))
}

async fn test_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<TestServerInput>,
) -> Result<Json<ConnectionCheck>, StatusCode> {
    check_auth(&headers, &state.vault).await?;

    let kek = match state.vault.get_kek().await {
        Ok(k) => k,
        Err(_) => {
            return Ok(Json(ConnectionCheck {
                ok: false,
                message: "Vault terkunci, silakan masukkan PIN kembali".to_string(),
            }));
        }
    };

    let (password, private_key, passphrase) = if let Some(id) = input.id {
        if let Ok(saved) = state.db.get_server_credentials(id, &kek) {
            (
                input.password.filter(|p| !p.is_empty()).or(saved.password),
                input.private_key.filter(|k| !k.is_empty()).or(saved.private_key),
                input.passphrase.filter(|p| !p.is_empty()).or(saved.passphrase),
            )
        } else {
            (input.password, input.private_key, input.passphrase)
        }
    } else {
        (input.password, input.private_key, input.passphrase)
    };

    let creds = DecryptedServerCreds {
        id: input.id.unwrap_or(0),
        name: input.name,
        host: input.host,
        port: input.port as u16,
        username: input.username,
        auth_type: input.auth_type,
        password,
        private_key,
        passphrase,
    };

    match state.pool.test_connection(&creds).await {
        Ok(_) => Ok(Json(ConnectionCheck {
            ok: true,
            message: "Koneksi SSH berhasil terhubung".to_string(),
        })),
        Err(e) => Ok(Json(ConnectionCheck {
            ok: false,
            message: e.to_string(),
        })),
    }
}

async fn server_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ServerStats>, (StatusCode, Json<serde_json::Value>)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, Json(serde_json::json!({ "detail": "Unauthorized" }))))?;

    let kek = state
        .vault
        .get_kek()
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "detail": e.to_string() }))))?;

    let creds = state
        .db
        .get_server_credentials(id, &kek)
        .map_err(|e| (StatusCode::NOT_FOUND, Json(serde_json::json!({ "detail": e.to_string() }))))?;

    let raw_output = state
        .pool
        .run_command(&creds, STATS_SCRIPT)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "detail": format!("Koneksi ke server gagal: {}", e)
                })),
            )
        })?;

    let stats = parse_stats(&raw_output);
    Ok(Json(stats))
}

async fn reset_host_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ServerOutput>, StatusCode> {
    check_auth(&headers, &state.vault).await?;
    state.pool.disconnect(id).await;
    state
        .db
        .reset_host_key(id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let servers = state.db.list_servers().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let server = servers.into_iter().find(|s| s.id == id).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(server))
}

type JsonError = (StatusCode, Json<Value>);

fn json_error(status: StatusCode, detail: impl Into<String>) -> JsonError {
    (status, Json(json!({ "detail": detail.into() })))
}

async fn authorized_creds(
    state: &AppState,
    headers: &HeaderMap,
    id: i64,
) -> Result<DecryptedServerCreds, JsonError> {
    check_auth(headers, &state.vault)
        .await
        .map_err(|s| json_error(s, "Unauthorized"))?;
    let kek = state
        .vault
        .get_kek()
        .await
        .map_err(|e| json_error(StatusCode::UNAUTHORIZED, e.to_string()))?;
    state
        .db
        .get_server_credentials(id, &kek)
        .map_err(|_| json_error(StatusCode::NOT_FOUND, "Server tidak ditemukan"))
}

#[derive(Deserialize)]
pub struct LsQuery {
    #[serde(default)]
    pub path: String,
}

/// Folder picker for saved paths; `~` and `~/sub` are resolved on the server.
async fn list_dir(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Query(query): Query<LsQuery>,
) -> Result<Json<Value>, JsonError> {
    let creds = authorized_creds(&state, &headers, id).await?;
    let path = query.path.trim();
    if path.contains('\0') {
        return Err(json_error(StatusCode::BAD_REQUEST, "Path tidak valid"));
    }
    let payload = json!({ "op": "ls", "path": if path.is_empty() { "~" } else { path } });
    state
        .pool
        .run_agent(&creds, &payload)
        .await
        .map(Json)
        .map_err(|e| json_error(StatusCode::BAD_REQUEST, e.to_string()))
}

/// Stores a clipboard image on the server and returns its path, which the terminal pastes
/// so CLIs such as Claude Code can attach it.
async fn paste_image(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    body: Bytes,
) -> Result<Json<Value>, JsonError> {
    let creds = authorized_creds(&state, &headers, id).await?;
    if body.len() > PASTE_MAX_BYTES {
        return Err(json_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("Gambar terlalu besar (maks {} MB)", PASTE_MAX_BYTES / (1024 * 1024)),
        ));
    }
    let ext = sniff_image_type(&body).ok_or_else(|| {
        json_error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Format gambar tidak didukung (hanya PNG, JPEG, GIF, WebP)",
        )
    })?;

    let name = paste_image_name(ext, chrono::Local::now().naive_local());
    let stdin = build_paste_request(&name, &body);
    let out = state
        .pool
        .run_command_with_stdin(&creds, &build_agent_command(), &stdin, 60)
        .await
        .map_err(|e| json_error(StatusCode::BAD_GATEWAY, format!("Koneksi ke server gagal: {}", e)))?;

    let parsed: Value = serde_json::from_slice(&out).map_err(|_| {
        json_error(
            StatusCode::BAD_GATEWAY,
            format!(
                "Respons server tidak valid (pastikan python3 terpasang): {}",
                String::from_utf8_lossy(&out).trim()
            ),
        )
    })?;
    if parsed.get("ok").and_then(Value::as_bool) != Some(true) {
        let detail = parsed
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("Gagal menyimpan gambar di server");
        return Err(json_error(StatusCode::BAD_GATEWAY, detail));
    }
    let path = parsed
        .pointer("/result/path")
        .and_then(Value::as_str)
        .ok_or_else(|| json_error(StatusCode::BAD_GATEWAY, "Respons server tanpa path"))?;
    Ok(Json(json!({ "path": path })))
}
