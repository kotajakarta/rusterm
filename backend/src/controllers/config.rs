use crate::controllers::servers::{check_auth, AppState};
use crate::models::server::{BackupEnvelope, BackupStats};
use crate::services::crypto::CryptoService;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ExportInput {
    pub pin: String,
}

#[derive(Deserialize)]
pub struct ImportInput {
    pub pin: String,
    pub backup: BackupEnvelope,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/config/export", post(export_config))
        .route("/config/import", post(import_config))
        .with_state(state)
}

async fn export_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ExportInput>,
) -> Result<Json<BackupEnvelope>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let kek = state
        .vault
        .get_kek()
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    let payload = state
        .db
        .export_backup_data(&kek)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let envelope = CryptoService::encrypt_backup(&payload, &input.pin)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(envelope))
}

async fn import_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ImportInput>,
) -> Result<Json<BackupStats>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let kek = state
        .vault
        .get_kek()
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    let payload = CryptoService::decrypt_backup(&input.backup, &input.pin)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let stats = state
        .db
        .import_backup_data(&payload, &kek)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(stats))
}
