use axum::body::Body;
use axum::http::{Request, StatusCode};
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::tempdir;
use tower::ServiceExt;
use xterm_backend::controllers::servers::{self, AppState};
use xterm_backend::models::db::DbStore;
use xterm_backend::models::server::ServerInput;
use xterm_backend::services::cloudflare::CloudflareClient;
use xterm_backend::services::paste_image::{
    build_paste_request, paste_image_name, sniff_image_type, PASTE_DIR, PASTE_KEEP_SECONDS,
    PASTE_MAX_BYTES,
};
use xterm_backend::services::ssh_pool::{SshPool, FILES_AGENT_SRC};
use xterm_backend::services::vault::Vault;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";

#[test]
fn test_sniff_image_type() {
    assert_eq!(sniff_image_type(PNG), Some("png"));
    assert_eq!(sniff_image_type(b"\xff\xd8\xff\xe0rest"), Some("jpg"));
    assert_eq!(sniff_image_type(b"GIF89a...."), Some("gif"));
    assert_eq!(sniff_image_type(b"GIF87a...."), Some("gif"));
    assert_eq!(sniff_image_type(b"RIFF\x10\x00\x00\x00WEBPVP8 "), Some("webp"));
    assert_eq!(sniff_image_type(b"<svg xmlns=...>"), None);
    assert_eq!(sniff_image_type(b""), None);
}

#[test]
fn test_paste_image_name() {
    let at = chrono::NaiveDate::from_ymd_opt(2026, 10, 19)
        .unwrap()
        .and_hms_opt(8, 5, 3)
        .unwrap();
    let name = paste_image_name("png", at);
    assert!(name.starts_with("paste-20261019-080503-"), "{}", name);
    assert!(name.ends_with(".png"));
    assert_eq!(name.len(), "paste-20261019-080503-abcdef.png".len());
    assert_ne!(paste_image_name("png", at), paste_image_name("png", at));
}

fn run_agent_locally(home: &std::path::Path, stdin: &[u8]) -> serde_json::Value {
    let mut child = Command::new("python3")
        .arg("-c")
        .arg(FILES_AGENT_SRC)
        .env("HOME", home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("python3 harus tersedia untuk test agent");
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    let out = child.wait_with_output().unwrap();
    serde_json::from_slice(&out.stdout).expect("output agent harus JSON")
}

#[test]
fn test_agent_paste_image_writes_private_file_and_prunes_stale() {
    use std::os::unix::fs::PermissionsExt;

    let home = tempdir().unwrap();
    let folder = home.path().join(PASTE_DIR);
    std::fs::create_dir_all(&folder).unwrap();
    let stale = folder.join("paste-20260101-000000-aaaaaa.png");
    let recent = folder.join("paste-20260101-000000-bbbbbb.png");
    let unrelated = folder.join("notes.txt");
    for p in [&stale, &recent, &unrelated] {
        std::fs::write(p, PNG).unwrap();
    }
    let old = std::time::SystemTime::now()
        - std::time::Duration::from_secs(PASTE_KEEP_SECONDS + 60);
    for p in [&stale, &unrelated] {
        std::fs::File::options()
            .write(true)
            .open(p)
            .unwrap()
            .set_modified(old)
            .unwrap();
    }

    let name = "paste-20261019-080503-123abc.png";
    let res = run_agent_locally(home.path(), &build_paste_request(name, PNG));
    assert_eq!(res["ok"], true, "{}", res);

    let written = folder.join(name);
    assert_eq!(res["result"]["path"], written.to_str().unwrap());
    assert_eq!(std::fs::read(&written).unwrap(), PNG);
    let mode = std::fs::metadata(&written).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    assert!(!stale.exists(), "paste lama harus dibersihkan");
    assert!(recent.exists() && unrelated.exists());
}

#[test]
fn test_agent_paste_image_rejects_bad_name() {
    let home = tempdir().unwrap();
    let res = run_agent_locally(home.path(), &build_paste_request("../evil.png", PNG));
    assert_eq!(res["ok"], false, "{}", res);
}

#[test]
fn test_agent_ls_expands_home_and_sorts_dirs_first() {
    let home = tempdir().unwrap();
    std::fs::create_dir(home.path().join("zeta")).unwrap();
    std::fs::create_dir(home.path().join("alpha")).unwrap();
    std::fs::create_dir(home.path().join(".hidden")).unwrap();
    std::fs::write(home.path().join("berkas.txt"), b"x").unwrap();
    std::os::unix::fs::symlink(home.path().join("alpha"), home.path().join("link-ke-alpha")).unwrap();

    let req = serde_json::to_vec(&serde_json::json!({ "op": "ls", "path": "~" })).unwrap();
    let res = run_agent_locally(home.path(), &req);
    assert_eq!(res["ok"], true, "{}", res);
    let result = &res["result"];
    assert_eq!(result["path"], home.path().to_str().unwrap());

    let entries = result["entries"].as_array().unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["alpha", "link-ke-alpha", "zeta", "berkas.txt"]);
    assert_eq!(entries[1]["is_dir"], true);
    assert_eq!(entries[1]["is_symlink"], true);
    assert_eq!(entries[3]["is_dir"], false);
    assert_eq!(
        entries[0]["path"],
        home.path().join("alpha").to_str().unwrap()
    );

    let sub = serde_json::to_vec(&serde_json::json!({ "op": "ls", "path": "~/alpha" })).unwrap();
    let res = run_agent_locally(home.path(), &sub);
    assert_eq!(res["result"]["path"], home.path().join("alpha").to_str().unwrap());

    let rel = serde_json::to_vec(&serde_json::json!({ "op": "ls", "path": "relatif" })).unwrap();
    assert_eq!(run_agent_locally(home.path(), &rel)["ok"], false);
}

async fn setup() -> (tempfile::TempDir, axum::Router, String, i64) {
    let dir = tempdir().unwrap();
    let vault = Vault::new(&dir.path().join("vault.json")).await.unwrap();
    let session = vault.setup_pin("123456").await.unwrap();
    let kek = vault.get_kek().await.unwrap();
    let db = DbStore::new(&dir.path().join("db.sqlite")).unwrap();
    let id = db
        .create_server(
            &ServerInput {
                name: "Web1".into(),
                host: "192.0.2.1".into(),
                port: 22,
                username: "root".into(),
                auth_type: "password".into(),
                password: Some("pass".into()),
                private_key: None,
                passphrase: None,
                group_name: "".into(),
                color: "".into(),
            },
            &kek,
        )
        .unwrap();
    let app = axum::Router::new().nest(
        "/api",
        servers::router(AppState {
            db,
            vault,
            pool: SshPool::new(),
            cf: CloudflareClient::new(),
            transfers: Default::default(),
        }),
    );
    (dir, app, session, id)
}

async fn post(app: &axum::Router, uri: &str, session: &str, body: Vec<u8>) -> StatusCode {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("X-Session", session)
                .header("Content-Type", "image/png")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn test_paste_image_route_validation() {
    let (_dir, app, session, id) = setup().await;
    let url = format!("/api/servers/{}/paste-image", id);

    assert_eq!(post(&app, &url, "", PNG.to_vec()).await, StatusCode::UNAUTHORIZED);
    assert_eq!(
        post(&app, "/api/servers/999/paste-image", &session, b"x".to_vec()).await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        post(&app, &url, &session, b"just text".to_vec()).await,
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
    let mut huge = PNG.to_vec();
    huge.resize(PASTE_MAX_BYTES + 1, 0);
    assert_eq!(post(&app, &url, &session, huge).await, StatusCode::PAYLOAD_TOO_LARGE);
    // Gambar 3 MB (di atas batas default axum 2 MB) harus lolos validasi body.
    let mut big_ok = PNG.to_vec();
    big_ok.resize(3 * 1024 * 1024, 0);
    let status = post(&app, &url, &session, big_ok).await;
    assert_ne!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_ne!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_ls_route_auth_and_not_found() {
    let (_dir, app, session, _id) = setup().await;
    let get = |uri: &str, session: &str| {
        Request::builder()
            .uri(uri)
            .header("X-Session", session)
            .body(Body::empty())
            .unwrap()
    };
    let res = app.clone().oneshot(get("/api/servers/1/ls?path=~", "")).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let res = app
        .clone()
        .oneshot(get("/api/servers/999/ls?path=~", &session))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let ct = res.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(ct.starts_with("application/json"));
}
