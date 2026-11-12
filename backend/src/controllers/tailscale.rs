use crate::controllers::servers::AppState;
use crate::models::tailscale::{
    SetAuthorizedIn, SetKeyExpiryIn, SetRoutesIn, SetTagsIn, TailscaleAccount, TailscaleAccountIn,
    TailscaleAccountUpdateIn, TailscaleAccountVerifyIn, TailscaleDevice, TailscaleDeviceRoutes,
    TailscaleVerifyOut,
};
use crate::services::vault::Vault;
use axum::{
    extract::{Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
    Json, Router,
};
use serde_json::json;

pub fn router(state: AppState) -> Router {
    Router::new()
        // Tailscale Accounts
        .route("/tailscale/accounts", get(list_accounts).post(create_account))
        .route("/tailscale/accounts/verify", post(verify_account_token))
        .route(
            "/tailscale/accounts/:id",
            put(update_account).delete(delete_account),
        )
        // Machines / Devices
        .route("/tailscale/accounts/:id/devices", get(list_devices))
        .route(
            "/tailscale/accounts/:id/devices/:device_id",
            get(get_device).delete(delete_device),
        )
        // Machine Routes
        .route(
            "/tailscale/accounts/:id/devices/:device_id/routes",
            get(get_device_routes).post(set_device_routes),
        )
        // Machine Settings
        .route(
            "/tailscale/accounts/:id/devices/:device_id/authorized",
            post(set_device_authorized),
        )
        .route(
            "/tailscale/accounts/:id/devices/:device_id/key",
            post(set_device_key_expiry),
        )
        .route(
            "/tailscale/accounts/:id/devices/:device_id/tags",
            post(set_device_tags),
        )
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
) -> Result<Json<Vec<TailscaleAccount>>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let accounts = state
        .db
        .list_tailscale_accounts()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(accounts))
}

async fn verify_account_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<TailscaleAccountVerifyIn>,
) -> Result<Json<TailscaleVerifyOut>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let tailnet = if body.tailnet.trim().is_empty() {
        "-".to_string()
    } else {
        body.tailnet.trim().to_string()
    };

    let res = state
        .ts
        .verify_token(body.api_key.trim(), &tailnet)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    Ok(Json(res))
}

async fn create_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<TailscaleAccountIn>,
) -> Result<Json<TailscaleAccount>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let name = body.name.trim();
    let api_key = body.api_key.trim();
    if name.is_empty() || api_key.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Name and API key cannot be empty".into()));
    }

    let tailnet = if body.tailnet.trim().is_empty() {
        "-".to_string()
    } else {
        body.tailnet.trim().to_string()
    };

    let acc = state
        .db
        .create_tailscale_account(name, api_key, &tailnet)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(acc))
}

async fn update_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<i64>,
    Json(body): Json<TailscaleAccountUpdateIn>,
) -> Result<Json<TailscaleAccount>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let name = body.name.trim();
    if name.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Name cannot be empty".into()));
    }

    let api_key = body.api_key.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty());
    let tailnet = body.tailnet.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty());

    let acc = state
        .db
        .update_tailscale_account(id, name, api_key, tailnet)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(acc))
}

async fn delete_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    state
        .db
        .delete_tailscale_account(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({ "success": true })))
}

async fn list_devices(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<i64>,
) -> Result<Json<Vec<TailscaleDevice>>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let (acc, token) = state
        .db
        .get_tailscale_account(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Tailscale account not found".into()))?;

    let devices = state
        .ts
        .list_devices(&token, &acc.tailnet)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

    Ok(Json(devices))
}

async fn get_device(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath((id, device_id)): AxumPath<(i64, String)>,
) -> Result<Json<TailscaleDevice>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let (_acc, token) = state
        .db
        .get_tailscale_account(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Tailscale account not found".into()))?;

    let device = state
        .ts
        .get_device(&token, &device_id)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

    Ok(Json(device))
}

async fn delete_device(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath((id, device_id)): AxumPath<(i64, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let (_acc, token) = state
        .db
        .get_tailscale_account(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Tailscale account not found".into()))?;

    state
        .ts
        .delete_device(&token, &device_id)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

    Ok(Json(json!({ "success": true })))
}

async fn get_device_routes(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath((id, device_id)): AxumPath<(i64, String)>,
) -> Result<Json<TailscaleDeviceRoutes>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let (_acc, token) = state
        .db
        .get_tailscale_account(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Tailscale account not found".into()))?;

    let routes = state
        .ts
        .get_device_routes(&token, &device_id)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

    Ok(Json(routes))
}

async fn set_device_routes(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath((id, device_id)): AxumPath<(i64, String)>,
    Json(body): Json<SetRoutesIn>,
) -> Result<Json<TailscaleDeviceRoutes>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let (_acc, token) = state
        .db
        .get_tailscale_account(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Tailscale account not found".into()))?;

    let routes = state
        .ts
        .set_device_routes(&token, &device_id, &body.routes)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

    Ok(Json(routes))
}

async fn set_device_authorized(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath((id, device_id)): AxumPath<(i64, String)>,
    Json(body): Json<SetAuthorizedIn>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let (_acc, token) = state
        .db
        .get_tailscale_account(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Tailscale account not found".into()))?;

    state
        .ts
        .set_device_authorized(&token, &device_id, body.authorized)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

    Ok(Json(json!({ "success": true })))
}

async fn set_device_key_expiry(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath((id, device_id)): AxumPath<(i64, String)>,
    Json(body): Json<SetKeyExpiryIn>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let (_acc, token) = state
        .db
        .get_tailscale_account(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Tailscale account not found".into()))?;

    state
        .ts
        .set_device_key_expiry(&token, &device_id, body.key_expiry_disabled)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

    Ok(Json(json!({ "success": true })))
}

async fn set_device_tags(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath((id, device_id)): AxumPath<(i64, String)>,
    Json(body): Json<SetTagsIn>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let (_acc, token) = state
        .db
        .get_tailscale_account(id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Tailscale account not found".into()))?;

    state
        .ts
        .set_device_tags(&token, &device_id, &body.tags)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

    Ok(Json(json!({ "success": true })))
}
