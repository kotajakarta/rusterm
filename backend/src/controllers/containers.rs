use crate::controllers::servers::AppState;
use crate::models::containers::{
    normalize_container_item, ContainerActionBody, ContainerEngineInfo, ContainerItem,
    ContainerLogsOut, ContainerRunBody, ImageItem, ImagePullBody,
};
use crate::models::server::DecryptedServerCreds;
use crate::services::vault::Vault;
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/containers/engine", get(get_engine))
        .route("/containers/list", get(list_containers))
        .route("/containers/action", post(container_action))
        .route("/containers/logs", get(container_logs))
        .route(
            "/containers/images",
            get(list_images).delete(delete_image),
        )
        .route("/containers/images/pull", post(pull_image))
        .route("/containers/run", post(run_container))
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

/// Runs a mutating engine command and fails with its stderr when the exit status is non-zero
/// (`run_command` ignores the exit status, so failures used to be reported as success).
async fn run_checked(
    state: &AppState,
    creds: &DecryptedServerCreds,
    cmd: &str,
    timeout_secs: u64,
) -> Result<String, (StatusCode, String)> {
    let out = state
        .pool
        .exec_capture(creds, cmd, &[], timeout_secs)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    if out.success() {
        Ok(out.stdout_text())
    } else {
        Err((StatusCode::BAD_REQUEST, out.failure_message()))
    }
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

async fn detect_engine(
    state: &AppState,
    creds: &DecryptedServerCreds,
) -> ContainerEngineInfo {
    // 1. Check podman
    if let Ok(out) = state
        .pool
        .run_command(creds, "podman --version 2>/dev/null")
        .await
    {
        let trimmed = out.trim();
        if trimmed.to_lowercase().contains("podman") {
            let version = trimmed
                .split_whitespace()
                .nth(2)
                .unwrap_or(trimmed)
                .to_string();

            let rootless_out = state
                .pool
                .run_command(
                    creds,
                    "podman info --format '{{.Host.Security.Rootless}}' 2>/dev/null",
                )
                .await
                .unwrap_or_default();
            let is_rootless = rootless_out.to_lowercase().contains("true");

            return ContainerEngineInfo {
                engine: "podman".into(),
                version,
                rootless: is_rootless,
                error: None,
            };
        }
    }

    // 2. Check docker
    if let Ok(out) = state
        .pool
        .run_command(creds, "docker --version 2>/dev/null")
        .await
    {
        let trimmed = out.trim();
        if trimmed.to_lowercase().contains("docker") {
            let version = trimmed
                .split_whitespace()
                .nth(2)
                .unwrap_or(trimmed)
                .trim_end_matches(',')
                .to_string();

            let sec_out = state
                .pool
                .run_command(
                    creds,
                    "docker info --format '{{.SecurityOptions}}' 2>/dev/null",
                )
                .await
                .unwrap_or_default();
            let is_rootless = sec_out.to_lowercase().contains("rootless");

            return ContainerEngineInfo {
                engine: "docker".into(),
                version,
                rootless: is_rootless,
                error: None,
            };
        }
    }

    ContainerEngineInfo {
        engine: "none".into(),
        version: "".into(),
        rootless: false,
        error: Some("Podman atau Docker tidak terdeteksi pada server remote ini.".into()),
    }
}

#[derive(Deserialize)]
struct ServerIdQuery {
    server_id: i64,
}

async fn get_engine(
    Query(query): Query<ServerIdQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ContainerEngineInfo>, (StatusCode, String)> {
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
        .get_server_credentials(query.server_id, &kek)
        .map_err(|_| (StatusCode::NOT_FOUND, "Server not found".into()))?;

    let info = detect_engine(&state, &creds).await;
    Ok(Json(info))
}

#[derive(Deserialize)]
struct ListContainersQuery {
    server_id: i64,
    #[serde(default = "default_true")]
    all: bool,
}

fn default_true() -> bool {
    true
}

async fn list_containers(
    Query(query): Query<ListContainersQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ContainerItem>>, (StatusCode, String)> {
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
        .get_server_credentials(query.server_id, &kek)
        .map_err(|_| (StatusCode::NOT_FOUND, "Server not found".into()))?;

    let eng_info = detect_engine(&state, &creds).await;
    if eng_info.engine == "none" {
        return Ok(Json(Vec::new()));
    }

    let all_flag = if query.all { "-a " } else { "" };
    let mut items = Vec::new();

    if eng_info.engine == "podman" {
        let cmd = format!("podman ps {}--format json", all_flag);
        let out = state
            .pool
            .run_command(&creds, &cmd)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let trimmed = out.trim();
        if !trimmed.is_empty() {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                if let Some(arr) = val.as_array() {
                    for v in arr {
                        items.push(normalize_container_item(v));
                    }
                } else if val.is_object() {
                    items.push(normalize_container_item(&val));
                }
            }
        }
    } else {
        let cmd = format!("docker ps {}--format '{{{{json .}}}}'", all_flag);
        let out = state
            .pool
            .run_command(&creds, &cmd)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        for line in out.lines() {
            let line = line.trim();
            if !line.is_empty() {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                    items.push(normalize_container_item(&val));
                }
            }
        }
    }

    Ok(Json(items))
}

async fn container_action(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ContainerActionBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
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
        .get_server_credentials(body.server_id, &kek)
        .map_err(|_| (StatusCode::NOT_FOUND, "Server not found".into()))?;

    let eng_info = detect_engine(&state, &creds).await;
    if eng_info.engine == "none" {
        return Err((
            StatusCode::BAD_REQUEST,
            eng_info.error.unwrap_or_else(|| "Engine not found".into()),
        ));
    }

    let valid_actions = ["start", "stop", "restart", "pause", "unpause", "delete"];
    if !valid_actions.contains(&body.action.as_str()) {
        return Err((StatusCode::BAD_REQUEST, "Invalid action".into()));
    }

    let cid_q = shell_quote(&body.container_id);
    let cmd = if body.action == "delete" {
        let flag = if body.force.unwrap_or(false) { "-f " } else { "" };
        format!("{} rm {}{}", eng_info.engine, flag, cid_q)
    } else {
        format!("{} {} {}", eng_info.engine, body.action, cid_q)
    };

    let out = run_checked(&state, &creds, &cmd, 60).await?;

    Ok(Json(json!({
        "ok": true,
        "message": format!("Kontainer {} berhasil di-{}", body.container_id, body.action),
        "output": out.trim(),
    })))
}

#[derive(Deserialize)]
struct LogsQuery {
    server_id: i64,
    container_id: String,
    #[serde(default = "default_tail")]
    tail: u32,
}

fn default_tail() -> u32 {
    150
}

async fn container_logs(
    Query(query): Query<LogsQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ContainerLogsOut>, (StatusCode, String)> {
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
        .get_server_credentials(query.server_id, &kek)
        .map_err(|_| (StatusCode::NOT_FOUND, "Server not found".into()))?;

    let eng_info = detect_engine(&state, &creds).await;
    if eng_info.engine == "none" {
        return Err((
            StatusCode::BAD_REQUEST,
            eng_info.error.unwrap_or_else(|| "Engine not found".into()),
        ));
    }

    let cid_q = shell_quote(&query.container_id);
    let cmd = format!("{} logs --tail {} {} 2>&1", eng_info.engine, query.tail, cid_q);
    let out = state
        .pool
        .run_command(&creds, &cmd)
        .await
        .unwrap_or_default();

    Ok(Json(ContainerLogsOut { logs: out }))
}

async fn list_images(
    Query(query): Query<ServerIdQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ImageItem>>, (StatusCode, String)> {
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
        .get_server_credentials(query.server_id, &kek)
        .map_err(|_| (StatusCode::NOT_FOUND, "Server not found".into()))?;

    let eng_info = detect_engine(&state, &creds).await;
    if eng_info.engine == "none" {
        return Ok(Json(Vec::new()));
    }

    let mut results = Vec::new();

    if eng_info.engine == "podman" {
        let cmd = "podman images --format json";
        if let Ok(out) = state.pool.run_command(&creds, cmd).await {
            let trimmed = out.trim();
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                let list = if let Some(arr) = val.as_array() {
                    arr.clone()
                } else if val.is_object() {
                    vec![val]
                } else {
                    Vec::new()
                };

                for img in list {
                    let iid_full = img
                        .get("Id")
                        .or_else(|| img.get("id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let id = if iid_full.len() > 12 {
                        iid_full[..12].to_string()
                    } else {
                        iid_full.to_string()
                    };

                    let mut repo = img
                        .get("Repository")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let mut tag = img
                        .get("Tag")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    if repo.is_empty() {
                        if let Some(names) = img.get("Names").and_then(|v| v.as_array()) {
                            if let Some(first) = names.first().and_then(|n| n.as_str()) {
                                if let Some((r, t)) = first.split_once(':') {
                                    repo = r.to_string();
                                    tag = t.to_string();
                                } else {
                                    repo = first.to_string();
                                }
                            }
                        }
                    }

                    let size_str = if let Some(bytes) = img.get("Size").and_then(|v| v.as_u64()) {
                        format!("{:.1} MB", (bytes as f64) / (1024.0 * 1024.0))
                    } else {
                        img.get("Size")
                            .and_then(|v| v.as_str())
                            .unwrap_or("<unknown>")
                            .to_string()
                    };

                    let created = img
                        .get("Created")
                        .or_else(|| img.get("CreatedAt"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    results.push(ImageItem {
                        id,
                        repository: if repo.is_empty() { "<none>".into() } else { repo },
                        tag: if tag.is_empty() { "<none>".into() } else { tag },
                        size: size_str,
                        created,
                    });
                }
            }
        }
    } else {
        let cmd = "docker images --format '{{json .}}'";
        if let Ok(out) = state.pool.run_command(&creds, cmd).await {
            for line in out.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) {
                    let iid_full = obj.get("ID").and_then(|v| v.as_str()).unwrap_or("");
                    let id = if iid_full.len() > 12 {
                        iid_full[..12].to_string()
                    } else {
                        iid_full.to_string()
                    };

                    results.push(ImageItem {
                        id,
                        repository: obj
                            .get("Repository")
                            .and_then(|v| v.as_str())
                            .unwrap_or("<none>")
                            .to_string(),
                        tag: obj
                            .get("Tag")
                            .and_then(|v| v.as_str())
                            .unwrap_or("<none>")
                            .to_string(),
                        size: obj
                            .get("Size")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        created: obj
                            .get("CreatedAt")
                            .or_else(|| obj.get("CreatedSince"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    });
                }
            }
        }
    }

    Ok(Json(results))
}

async fn pull_image(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ImagePullBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
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
        .get_server_credentials(body.server_id, &kek)
        .map_err(|_| (StatusCode::NOT_FOUND, "Server not found".into()))?;

    let eng_info = detect_engine(&state, &creds).await;
    if eng_info.engine == "none" {
        return Err((
            StatusCode::BAD_REQUEST,
            eng_info.error.unwrap_or_else(|| "Engine not found".into()),
        ));
    }

    let img_q = shell_quote(body.image.trim());
    let cmd = format!("{} pull {}", eng_info.engine, img_q);
    let out = run_checked(&state, &creds, &cmd, 900).await?;

    Ok(Json(json!({
        "ok": true,
        "message": format!("Image {} berhasil diunduh.", body.image),
        "output": out.trim(),
    })))
}

#[derive(Deserialize)]
struct DeleteImageQuery {
    server_id: i64,
    image_id: String,
    #[serde(default)]
    force: bool,
}

async fn delete_image(
    Query(query): Query<DeleteImageQuery>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
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
        .get_server_credentials(query.server_id, &kek)
        .map_err(|_| (StatusCode::NOT_FOUND, "Server not found".into()))?;

    let eng_info = detect_engine(&state, &creds).await;
    if eng_info.engine == "none" {
        return Err((
            StatusCode::BAD_REQUEST,
            eng_info.error.unwrap_or_else(|| "Engine not found".into()),
        ));
    }

    let flag = if query.force { "-f " } else { "" };
    let img_q = shell_quote(&query.image_id);
    let cmd = format!("{} rmi {}{}", eng_info.engine, flag, img_q);

    let out = run_checked(&state, &creds, &cmd, 120).await?;

    Ok(Json(json!({
        "ok": true,
        "message": format!("Image {} berhasil dihapus.", query.image_id),
        "output": out.trim(),
    })))
}

async fn run_container(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ContainerRunBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
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
        .get_server_credentials(body.server_id, &kek)
        .map_err(|_| (StatusCode::NOT_FOUND, "Server not found".into()))?;

    let eng_info = detect_engine(&state, &creds).await;
    if eng_info.engine == "none" {
        return Err((
            StatusCode::BAD_REQUEST,
            eng_info.error.unwrap_or_else(|| "Engine not found".into()),
        ));
    }

    let mut parts = vec![eng_info.engine, "run".into(), "-d".into()];
    if let Some(ref name) = body.name {
        if !name.trim().is_empty() {
            parts.push("--name".into());
            parts.push(shell_quote(name.trim()));
        }
    }
    for p in &body.ports {
        if !p.trim().is_empty() {
            parts.push("-p".into());
            parts.push(shell_quote(p.trim()));
        }
    }
    for e in &body.env {
        if !e.trim().is_empty() {
            parts.push("-e".into());
            parts.push(shell_quote(e.trim()));
        }
    }
    for v in &body.volumes {
        if !v.trim().is_empty() {
            parts.push("-v".into());
            parts.push(shell_quote(v.trim()));
        }
    }
    if let Some(ref r) = body.restart {
        if !r.trim().is_empty() {
            parts.push("--restart".into());
            parts.push(shell_quote(r.trim()));
        }
    }

    parts.push(shell_quote(body.image.trim()));
    if let Some(ref c) = body.command {
        if !c.trim().is_empty() {
            parts.push(c.trim().to_string());
        }
    }

    let cmd = parts.join(" ");
    let out = run_checked(&state, &creds, &cmd, 600).await?;

    let new_cid = out.trim();
    let short_cid = if new_cid.len() > 12 {
        &new_cid[..12]
    } else {
        new_cid
    };

    Ok(Json(json!({
        "ok": true,
        "id": short_cid,
        "message": format!("Kontainer {} berhasil dijalankan", short_cid),
    })))
}
