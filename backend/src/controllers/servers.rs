use crate::models::db::DbStore;
use crate::models::server::{
    ConnectionCheck, DecryptedServerCreds, ServerInput, ServerOutput, ServerStats,
    TestServerInput,
};
use crate::services::ssh_pool::{parse_stats, SshPool, STATS_SCRIPT};
use crate::services::vault::Vault;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;

#[derive(Clone)]
pub struct AppState {
    pub db: DbStore,
    pub vault: Vault,
    pub pool: SshPool,
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
) -> Result<Json<ServerStats>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let kek = state
        .vault
        .get_kek()
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    let creds = state
        .db
        .get_server_credentials(id, &kek)
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;

    let raw_output = state
        .pool
        .run_command(&creds, STATS_SCRIPT)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("SSH command failed: {}", e)))?;

    let stats = parse_stats(&raw_output);
    Ok(Json(stats))
}

async fn reset_host_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ServerOutput>, StatusCode> {
    check_auth(&headers, &state.vault).await?;
    state
        .db
        .reset_host_key(id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let servers = state.db.list_servers().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let server = servers.into_iter().find(|s| s.id == id).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(server))
}
