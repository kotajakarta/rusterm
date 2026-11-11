use crate::controllers::servers::{check_auth, AppState};
use crate::services::vault::{AuthStatus, VaultError};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct PinPayload {
    pub pin: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub session: String,
    pub must_change_pin: bool,
}

#[derive(Deserialize)]
pub struct ChangePinPayload {
    pub old_pin: String,
    pub new_pin: String,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/status", get(status_handler))
        .route("/setup-pin", post(setup_pin_handler))
        .route("/login", post(login_handler))
        .route("/change-pin", post(change_pin_handler))
        .route("/lock", post(lock_handler))
        .with_state(state)
}

async fn status_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Json<AuthStatus> {
    let session = headers
        .get("X-Session")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    Json(state.vault.status(session).await)
}

async fn setup_pin_handler(
    State(state): State<AppState>,
    Json(payload): Json<PinPayload>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    match state.vault.setup_pin(&payload.pin).await {
        Ok(session) => Ok(Json(LoginResponse {
            session,
            must_change_pin: false,
        })),
        Err(e) => Err((StatusCode::BAD_REQUEST, e.to_string())),
    }
}

async fn login_handler(
    State(state): State<AppState>,
    Json(payload): Json<PinPayload>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    match state.vault.login(&payload.pin).await {
        Ok(session) => Ok(Json(LoginResponse {
            session,
            must_change_pin: false,
        })),
        Err(e @ VaultError::TooManyAttempts(_)) => Err((StatusCode::TOO_MANY_REQUESTS, e.to_string())),
        Err(e) => Err((StatusCode::UNAUTHORIZED, e.to_string())),
    }
}

async fn change_pin_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ChangePinPayload>,
) -> Result<StatusCode, (StatusCode, String)> {
    let session = headers
        .get("X-Session")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !state.vault.is_session_valid(session).await {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".to_string()));
    }

    match state
        .vault
        .change_pin(&payload.old_pin, &payload.new_pin, &state.db)
        .await
    {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err((StatusCode::BAD_REQUEST, e.to_string())),
    }
}

async fn lock_handler(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if check_auth(&headers, &state.vault).await.is_err() {
        return StatusCode::UNAUTHORIZED;
    }
    state.vault.lock().await;
    state.pool.disconnect_all().await;
    StatusCode::NO_CONTENT
}
