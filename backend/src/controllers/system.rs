use crate::controllers::servers::{check_auth, AppState};
pub use crate::services::updater::UpdateProgressResult;
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

#[derive(Deserialize, Debug)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize, Debug)]
struct GitHubRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    #[allow(dead_code)]
    html_url: String,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
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

                // Pick the direct binary asset URL if available
                #[cfg(target_os = "windows")]
                let asset_url = release
                    .assets
                    .iter()
                    .find(|a| a.name.eq_ignore_ascii_case("rusterm.exe"))
                    .map(|a| a.browser_download_url.clone())
                    .unwrap_or_else(|| {
                        format!(
                            "https://github.com/kotajakarta/rusterm/releases/download/{}/rusterm.exe",
                            release.tag_name
                        )
                    });

                #[cfg(not(target_os = "windows"))]
                let asset_url = release
                    .assets
                    .iter()
                    .find(|a| a.name == "rusterm")
                    .map(|a| a.browser_download_url.clone())
                    .unwrap_or_else(|| {
                        format!(
                            "https://github.com/kotajakarta/rusterm/releases/download/{}/rusterm.exe",
                            release.tag_name
                        )
                    });

                return Ok(Json(UpdateCheckResult {
                    platform: "desktop".into(),
                    current_version,
                    latest_version: latest_tag.to_string(),
                    has_update,
                    release_title: release.name.or(Some(format!("RusTerm {}", release.tag_name))),
                    release_notes: release.body,
                    download_url: Some(asset_url),
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

    let current = state.updater.get_progress().await;
    if current.state == "downloading" || current.state == "applying" {
        return Ok(Json(UpdateApplyResult {
            started: true,
            message: "Pembaruan sedang berjalan di latar belakang.".into(),
        }));
    }

    #[cfg(target_os = "windows")]
    let default_url = "https://github.com/kotajakarta/rusterm/releases/latest/download/rusterm.exe".to_string();
    #[cfg(not(target_os = "windows"))]
    let default_url = "https://github.com/kotajakarta/rusterm/releases/latest/download/rusterm".to_string();

    let updater = std::sync::Arc::new(state.updater.clone());
    tokio::spawn(async move {
        if let Err(e) = crate::services::updater::run_in_app_update(updater, default_url).await {
            tracing::error!("In-app self-update failed: {}", e);
        }
    });

    Ok(Json(UpdateApplyResult {
        started: true,
        message: "Memulai proses pembaruan otomatis di latar belakang.".into(),
    }))
}

async fn update_progress(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UpdateProgressResult>, (StatusCode, String)> {
    check_auth(&headers, &state.vault)
        .await
        .map_err(|s| (s, "Unauthorized".into()))?;

    let progress = state.updater.get_progress().await;
    Ok(Json(progress))
}
