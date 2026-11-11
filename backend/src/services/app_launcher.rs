use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::info;

pub fn resolve_data_dir(cli_dir: Option<PathBuf>, exe_path: Option<PathBuf>) -> PathBuf {
    if let Some(dir) = cli_dir {
        return dir;
    }

    if let Some(path) = exe_path {
        if let Some(parent) = path.parent() {
            return parent.join("conf");
        }
    }

    let local = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    if local.join("rusterm").exists() {
        local.join("rusterm")
    } else if local.join("xterm-rust").exists() {
        local.join("xterm-rust")
    } else if local.join("TermDeck").exists() {
        local.join("TermDeck")
    } else {
        local.join("RusTerm")
    }
}

pub fn build_app_args(port: u16, profile_dir: &Path) -> Vec<String> {
    vec![
        format!("--app=http://127.0.0.1:{}", port),
        format!("--user-data-dir={}", profile_dir.display()),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
        "--disable-features=msStartupBoost".to_string(),
    ]
}

pub async fn check_existing_instance(port: u16) -> bool {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(600))
        .build()
        .unwrap_or_default();
    let url = format!("http://127.0.0.1:{}/api/auth/status", port);
    match client.get(&url).send().await {
        Ok(res) => res.status().is_success(),
        Err(_) => false,
    }
}

pub fn find_browser_executable() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let prog_files = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".to_string());
        let prog_files_x86 = std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| "C:\\Program Files (x86)".to_string());
        let local_app_data = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("C:\\Users\\Default\\AppData\\Local"));

        let candidates = [
            // 1. Edge
            PathBuf::from(&prog_files_x86).join("Microsoft\\Edge\\Application\\msedge.exe"),
            PathBuf::from(&prog_files).join("Microsoft\\Edge\\Application\\msedge.exe"),
            local_app_data.join("Microsoft\\Edge\\Application\\msedge.exe"),
            // 2. Chrome
            PathBuf::from(&prog_files).join("Google\\Chrome\\Application\\chrome.exe"),
            PathBuf::from(&prog_files_x86).join("Google\\Chrome\\Application\\chrome.exe"),
            local_app_data.join("Google\\Chrome\\Application\\chrome.exe"),
            // 3. Brave
            PathBuf::from(&prog_files).join("BraveSoftware\\Brave-Browser\\Application\\brave.exe"),
            local_app_data.join("BraveSoftware\\Brave-Browser\\Application\\brave.exe"),
        ];

        for c in &candidates {
            if c.is_file() {
                return Some(c.clone());
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let linux_candidates = [
            "/usr/bin/google-chrome",
            "/usr/bin/google-chrome-stable",
            "/usr/bin/chromium",
            "/usr/bin/chromium-browser",
            "/usr/bin/brave-browser",
            "/usr/bin/microsoft-edge",
            "/usr/bin/microsoft-edge-stable",
            "/snap/bin/chromium",
        ];
        for path_str in &linux_candidates {
            let path = PathBuf::from(path_str);
            if path.is_file() {
                return Some(path);
            }
        }
    }

    None
}

pub fn launch_app_window(port: u16, data_dir: &Path) -> bool {
    let profile_dir = data_dir.join("browser");
    if let Err(e) = std::fs::create_dir_all(&profile_dir) {
        tracing::warn!("Gagal membuat folder profil browser: {}", e);
    }

    if let Some(browser_exe) = find_browser_executable() {
        let args = build_app_args(port, &profile_dir);
        info!("Meluncurkan jendela aplikasi mandiri: {:?} {:?}", browser_exe, args);

        let mut cmd = Command::new(&browser_exe);
        cmd.args(&args);

        match cmd.spawn() {
            Ok(_) => true,
            Err(e) => {
                tracing::warn!("Gagal meluncurkan browser: {}", e);
                false
            }
        }
    } else {
        // Fallback to default browser
        let url = format!("http://127.0.0.1:{}", port);
        info!("Browser Chromium tidak ditemukan, membuka URL di browser default: {}", url);
        #[cfg(target_os = "windows")]
        {
            let _ = Command::new("cmd").args(&["/c", "start", "", &url]).spawn();
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = Command::new("xdg-open").arg(&url).spawn();
        }
        true
    }
}
