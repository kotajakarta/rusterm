use crate::controllers::servers::{check_auth, AppState};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateCheckResult {
    pub platform: String,
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_title: Option<String>,
    pub release_notes: Option<String>,
    pub download_url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateApplyResult {
    pub started: bool,
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateProgressResult {
    pub state: String,
    pub progress_percent: u32,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub message: String,
}

#[derive(Deserialize, Debug)]
struct GitHubRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    html_url: String,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/system/update-check", get(check_update))
        .route("/system/update-apply", post(apply_update))
        .route("/system/update-progress", get(update_progress))
        .with_state(state)
}

async fn check_update(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UpdateCheckResult>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let release_repo_url = "https://github.com/kotajakarta/rusterm/releases".to_string();

    // Query GitHub API for latest release
    let client = reqwest::Client::builder()
        .user_agent(concat!("rusterm-app/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let gh_res = client
        .get("https://api.github.com/repos/kotajakarta/rusterm/releases/latest")
        .send()
        .await;

    match gh_res {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(release) = resp.json::<GitHubRelease>().await {
                let latest_tag = release.tag_name.trim_start_matches('v');
                let has_update = latest_tag != current_version;
                return Ok(Json(UpdateCheckResult {
                    platform: "desktop".into(),
                    current_version,
                    latest_version: latest_tag.to_string(),
                    has_update,
                    release_title: release.name.or(Some(format!("RusTerm {}", release.tag_name))),
                    release_notes: release.body,
                    download_url: Some(release.html_url),
                }));
            }
        }
        _ => {
            info!("Could not reach GitHub API or rate limited, using fallback release info");
        }
    }

    // Fallback if offline or rate limited
    Ok(Json(UpdateCheckResult {
        platform: "desktop".into(),
        current_version: current_version.clone(),
        latest_version: current_version,
        has_update: false,
        release_title: Some(format!("RusTerm v{}", env!("CARGO_PKG_VERSION"))),
        release_notes: Some("Rilis Windows mandiri (.exe) tersedia di https://github.com/kotajakarta/rusterm/releases".into()),
        download_url: Some(release_repo_url),
    }))
}

async fn apply_update(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UpdateApplyResult>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    Ok(Json(UpdateApplyResult {
        started: false,
        message: "Untuk versi desktop, silakan unduh berkas eksekusi (.exe) terbaru langsung dari rilis GitHub https://github.com/kotajakarta/rusterm/releases.".into(),
    }))
}

async fn update_progress(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UpdateProgressResult>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    Ok(Json(UpdateProgressResult {
        state: "idle".into(),
        progress_percent: 0,
        downloaded_bytes: 0,
        total_bytes: 0,
        message: "".into(),
    }))
}
