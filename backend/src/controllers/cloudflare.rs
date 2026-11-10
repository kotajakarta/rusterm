use crate::controllers::servers::AppState;
use crate::models::cloudflare::{
    CFAccount, CFAccountIn, CFAccountUpdateIn, CFAccountVerifyIn, CFRoute, CFRouteIn,
    CFSyncLocalCounts, CFSyncMetadata, CFSyncNowIn, CFSyncNowOut, CFSyncPullIn, CFSyncPullOut,
    CFSyncPushIn, CFSyncPushOut, CFSyncStatusOut, CFTunnel, CFZone,
};
use crate::models::server::BackupEnvelope;
use crate::services::crypto::CryptoService;
use crate::services::vault::Vault;
use axum::{
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

pub fn router(state: AppState) -> Router {
    Router::new()
        // Cloudflare Accounts
        .route("/cf/accounts", get(list_accounts).post(create_account))
        .route("/cf/accounts/verify", post(verify_account_token))
        .route(
            "/cf/accounts/:id",
            put(update_account).delete(delete_account),
        )
        // Tunnels & Routes
        .route("/cf/accounts/:id/tunnels", get(list_account_tunnels))
        .route("/cf/accounts/:id/zones", get(list_account_zones))
        .route(
            "/cf/accounts/:id/tunnels/:tunnel_id/routes",
            get(list_routes).post(create_route).delete(delete_route),
        )
        .route(
            "/cf/accounts/:id/tunnels/:tunnel_id/token",
            get(get_tunnel_token),
        )
        // KV Sync
        .route("/cf/sync/status", get(sync_status))
        .route("/cf/sync/sync-now", post(sync_now))
        .route("/cf/sync/push", post(sync_push))
        .route("/cf/sync/pull", post(sync_pull))
        .with_state(state)
}

async fn check_auth(headers: &HeaderMap, vault: &Vault) -> Result<(), StatusCode> {
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

async fn list_accounts(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<CFAccount>>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let accounts = state
        .db
        .list_cf_accounts()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(accounts))
}

async fn verify_account_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CFAccountVerifyIn>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let (account_id, account_name, has_dns_access) = state
        .cf
        .verify_token(body.token.trim())
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    Ok(Json(json!({
        "account_id": account_id,
        "account_name": account_name,
        "has_dns_access": has_dns_access,
    })))
}

async fn create_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CFAccountIn>,
) -> Result<(StatusCode, Json<CFAccount>), (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let token = body.token.trim();
    let (account_id, account_name, _) = state
        .cf
        .verify_token(token)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let acc = state
        .db
        .create_cf_account(body.name.trim(), token, &account_id, &account_name)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(acc)))
}

async fn update_account(
    AxumPath(id): AxumPath<i64>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CFAccountUpdateIn>,
) -> Result<Json<CFAccount>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let token_opt = body.token.as_deref().map(|t| t.trim());
    let acc = state
        .db
        .update_cf_account(id, body.name.trim(), token_opt)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(acc))
}

async fn delete_account(
    AxumPath(id): AxumPath<i64>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    state
        .db
        .delete_cf_account(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({ "ok": true })))
}

async fn list_account_tunnels(
    AxumPath(id): AxumPath<i64>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<CFTunnel>>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let (acc, token) = state
        .db
        .get_cf_account(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Akun tidak ditemukan".into()))?;

    let tunnels = state
        .cf
        .list_tunnels(&acc.account_id, &token)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    Ok(Json(tunnels))
}

async fn list_account_zones(
    AxumPath(id): AxumPath<i64>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<CFZone>>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let (acc, token) = state
        .db
        .get_cf_account(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Akun tidak ditemukan".into()))?;

    let zones = state
        .cf
        .list_zones(&acc.account_id, &token)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    Ok(Json(zones))
}

async fn list_routes(
    AxumPath((id, tunnel_id)): AxumPath<(i64, String)>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<CFRoute>>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let (acc, token) = state
        .db
        .get_cf_account(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Akun tidak ditemukan".into()))?;

    let routes = state
        .cf
        .list_routes(&acc.account_id, &tunnel_id, &token)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    Ok(Json(routes))
}

async fn create_route(
    AxumPath((id, tunnel_id)): AxumPath<(i64, String)>,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CFRouteIn>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let (acc, token) = state
        .db
        .get_cf_account(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Akun tidak ditemukan".into()))?;

    let res = state
        .cf
        .add_route(&acc.account_id, &tunnel_id, &token, &body)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    Ok(Json(res))
}

#[derive(Deserialize)]
struct DeleteRouteQuery {
    hostname: String,
    #[serde(default)]
    zone_id: Option<String>,
}

async fn delete_route(
    AxumPath((id, tunnel_id)): AxumPath<(i64, String)>,
    Query(query): Query<DeleteRouteQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let (acc, token) = state
        .db
        .get_cf_account(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Akun tidak ditemukan".into()))?;

    state
        .cf
        .delete_route(
            &acc.account_id,
            &tunnel_id,
            &token,
            &query.hostname,
            query.zone_id.as_deref(),
        )
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    Ok(Json(json!({
        "ok": true,
        "deleted": query.hostname
    })))
}

async fn get_tunnel_token(
    AxumPath((id, tunnel_id)): AxumPath<(i64, String)>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let (acc, token) = state
        .db
        .get_cf_account(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Akun tidak ditemukan".into()))?;

    let tok = state
        .cf
        .get_tunnel_token(&acc.account_id, &tunnel_id, &token)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    Ok(Json(json!({ "token": tok })))
}

// KV Sync Endpoints
#[derive(Deserialize)]
struct SyncStatusQuery {
    #[serde(default)]
    account_id: Option<i64>,
}

async fn sync_status(
    Query(query): Query<SyncStatusQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<CFSyncStatusOut>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let servers = state.db.list_servers().unwrap_or_default();
    let snippets = state.db.list_snippets().unwrap_or_default();
    let local_counts = CFSyncLocalCounts {
        servers_count: servers.len(),
        snippets_count: snippets.len(),
    };

    let accounts = state.db.list_cf_accounts().unwrap_or_default();
    if accounts.is_empty() {
        return Ok(Json(CFSyncStatusOut {
            configured: false,
            has_remote: false,
            remote_metadata: None,
            local_counts,
            message: Some("Belum ada akun Cloudflare yang terdaftar.".into()),
        }));
    }

    let target_id = query.account_id.unwrap_or(accounts[0].id);
    let (acc, token) = match state.db.get_cf_account(target_id) {
        Ok(Some(pair)) => pair,
        _ => {
            return Ok(Json(CFSyncStatusOut {
                configured: false,
                has_remote: false,
                remote_metadata: None,
                local_counts,
                message: Some("Akun Cloudflare tidak ditemukan.".into()),
            }))
        }
    };

    match state
        .cf
        .ensure_kv_namespace(&acc.account_id, &token, "termdeck_sync")
        .await
    {
        Ok(ns_id) => {
            let meta = state
                .cf
                .get_kv_metadata(&acc.account_id, &ns_id, "termdeck_vault", &token)
                .await
                .unwrap_or(None);

            let has_remote = meta.is_some();
            Ok(Json(CFSyncStatusOut {
                configured: true,
                has_remote,
                remote_metadata: meta,
                local_counts,
                message: None,
            }))
        }
        Err(e) => Ok(Json(CFSyncStatusOut {
            configured: true,
            has_remote: false,
            remote_metadata: None,
            local_counts,
            message: Some(format!("Koneksi KV Cloudflare gagal: {}", e)),
        })),
    }
}

async fn sync_push(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CFSyncPushIn>,
) -> Result<Json<CFSyncPushOut>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let accounts = state
        .db
        .list_cf_accounts()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if accounts.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Belum ada akun Cloudflare yang terdaftar.".into(),
        ));
    }

    let target_id = body.account_id.unwrap_or(accounts[0].id);
    let (acc, token) = state
        .db
        .get_cf_account(target_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Akun tidak ditemukan".into()))?;

    let kek = state
        .vault
        .get_kek()
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    let backup_payload = state
        .db
        .export_backup_data(&kek)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let envelope = CryptoService::encrypt_backup(&backup_payload, &body.pin)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let now_ts = chrono::Utc::now().timestamp();
    let meta = CFSyncMetadata {
        device_name: body.device_name,
        updated_at: now_ts,
        servers_count: backup_payload.servers.len(),
        snippets_count: backup_payload.snippets.len(),
        version: 1,
    };

    let ns_id = state
        .cf
        .ensure_kv_namespace(&acc.account_id, &token, "termdeck_sync")
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let env_val = serde_json::to_value(&envelope)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    state
        .cf
        .put_kv_value(&acc.account_id, &ns_id, "termdeck_vault", &token, &env_val, &meta)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    Ok(Json(CFSyncPushOut {
        success: true,
        synced_at: now_ts,
        remote_metadata: meta,
    }))
}

async fn sync_pull(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CFSyncPullIn>,
) -> Result<Json<CFSyncPullOut>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let accounts = state
        .db
        .list_cf_accounts()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if accounts.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Belum ada akun Cloudflare yang terdaftar.".into(),
        ));
    }

    let target_id = body.account_id.unwrap_or(accounts[0].id);
    let (acc, token) = state
        .db
        .get_cf_account(target_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Akun tidak ditemukan".into()))?;

    let ns_id = state
        .cf
        .ensure_kv_namespace(&acc.account_id, &token, "termdeck_sync")
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let remote_meta = state
        .cf
        .get_kv_metadata(&acc.account_id, &ns_id, "termdeck_vault", &token)
        .await
        .unwrap_or(None);

    let remote_val = state
        .cf
        .get_kv_value(&acc.account_id, &ns_id, "termdeck_vault", &token)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    if remote_val.is_none() {
        return Ok(Json(CFSyncPullOut {
            success: false,
            stats: Default::default(),
            remote_metadata: None,
        }));
    }

    let envelope: BackupEnvelope = serde_json::from_value(remote_val.unwrap())
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Invalid remote backup: {}", e)))?;

    let remote_payload = CryptoService::decrypt_backup(&envelope, &body.pin)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("PIN sync salah: {}", e)))?;

    let kek = state
        .vault
        .get_kek()
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    let stats = state
        .db
        .import_backup_data(&remote_payload, &kek)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(CFSyncPullOut {
        success: true,
        stats,
        remote_metadata: remote_meta,
    }))
}

async fn sync_now(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CFSyncNowIn>,
) -> Result<Json<CFSyncNowOut>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let accounts = state
        .db
        .list_cf_accounts()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if accounts.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Belum ada akun Cloudflare yang terdaftar.".into(),
        ));
    }

    let target_id = body.account_id.unwrap_or(accounts[0].id);
    let (acc, token) = state
        .db
        .get_cf_account(target_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Akun tidak ditemukan".into()))?;

    let ns_id = state
        .cf
        .ensure_kv_namespace(&acc.account_id, &token, "termdeck_sync")
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let kek = state
        .vault
        .get_kek()
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    // 1. Pull & Merge from remote if exists
    let mut stats = crate::models::server::BackupStats::default();
    if let Ok(Some(remote_val)) = state
        .cf
        .get_kv_value(&acc.account_id, &ns_id, "termdeck_vault", &token)
        .await
    {
        if let Ok(envelope) = serde_json::from_value::<BackupEnvelope>(remote_val) {
            if let Ok(remote_payload) = CryptoService::decrypt_backup(&envelope, &body.pin) {
                stats = state
                    .db
                    .import_backup_data(&remote_payload, &kek)
                    .unwrap_or_default();
            }
        }
    }

    // 2. Export unified & push back
    let unified_backup = state
        .db
        .export_backup_data(&kek)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let envelope = CryptoService::encrypt_backup(&unified_backup, &body.pin)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let now_ts = chrono::Utc::now().timestamp();
    let meta = CFSyncMetadata {
        device_name: body.device_name,
        updated_at: now_ts,
        servers_count: unified_backup.servers.len(),
        snippets_count: unified_backup.snippets.len(),
        version: 1,
    };

    let env_val = serde_json::to_value(&envelope)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    state
        .cf
        .put_kv_value(&acc.account_id, &ns_id, "termdeck_vault", &token, &env_val, &meta)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    Ok(Json(CFSyncNowOut {
        success: true,
        synced_at: now_ts,
        stats,
        remote_metadata: meta,
    }))
}
