use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateProgressResult {
    pub state: String,
    pub progress_percent: u32,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub message: String,
}

impl Default for UpdateProgressResult {
    fn default() -> Self {
        Self {
            state: "idle".to_string(),
            progress_percent: 0,
            downloaded_bytes: 0,
            total_bytes: 0,
            message: String::new(),
        }
    }
}

#[derive(Clone)]
pub struct UpdateProgress {
    inner: Arc<RwLock<UpdateProgressResult>>,
}

impl Default for UpdateProgress {
    fn default() -> Self {
        Self::new()
    }
}

impl UpdateProgress {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(UpdateProgressResult::default())),
        }
    }

    pub async fn get_progress(&self) -> UpdateProgressResult {
        self.inner.read().await.clone()
    }

    pub async fn set_downloading(&self, downloaded: u64, total: u64, message: &str) {
        let mut w = self.inner.write().await;
        w.state = "downloading".to_string();
        w.downloaded_bytes = downloaded;
        w.total_bytes = total;
        w.progress_percent = if total > 0 {
            ((downloaded as f64 / total as f64) * 100.0).min(100.0) as u32
        } else {
            0
        };
        w.message = message.to_string();
    }

    pub async fn set_applying(&self, message: &str) {
        let mut w = self.inner.write().await;
        w.state = "applying".to_string();
        w.progress_percent = 100;
        w.message = message.to_string();
    }

    pub async fn set_completed(&self, message: &str) {
        let mut w = self.inner.write().await;
        w.state = "completed".to_string();
        w.progress_percent = 100;
        w.message = message.to_string();
    }

    pub async fn set_error(&self, message: &str) {
        let mut w = self.inner.write().await;
        w.state = "error".to_string();
        w.message = message.to_string();
    }

    pub async fn reset(&self) {
        let mut w = self.inner.write().await;
        *w = UpdateProgressResult::default();
    }
}

pub fn validate_executable_bytes(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() < 1024 {
        return Err("Ukuran file terlalu kecil atau unduhan tidak lengkap".to_string());
    }

    // Windows PE binary starts with "MZ" (0x4D, 0x5A)
    let is_pe = bytes.len() >= 2 && bytes[0] == b'M' && bytes[1] == b'Z';
    // Linux ELF binary starts with 0x7F, 'E', 'L', 'F'
    let is_elf = bytes.len() >= 4 && bytes[0] == 0x7f && bytes[1] == b'E' && bytes[2] == b'L' && bytes[3] == b'F';

    if is_pe || is_elf {
        Ok(())
    } else {
        Err("Header berkas tidak valid untuk executable (bukan PE atau ELF)".to_string())
    }
}

pub fn kill_other_instances() {
    let current_pid = std::process::id();
    info!("Memeriksa dan mematikan instans lain di background selain PID: {}", current_pid);

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "rusterm.exe", "/FI", &format!("PID ne {}", current_pid)])
            .output();

        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "termdeck.exe", "/FI", &format!("PID ne {}", current_pid)])
            .output();
    }

    #[cfg(not(target_os = "windows"))]
    {
        use std::process::Command;
        if let Ok(output) = Command::new("pgrep").arg("-x").arg("rusterm").output() {
            let pids_str = String::from_utf8_lossy(&output.stdout);
            for line in pids_str.lines() {
                if let Ok(pid) = line.trim().parse::<u32>() {
                    if pid != current_pid {
                        let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
                    }
                }
            }
        }
    }
}

pub fn cleanup_stale_old_files(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    if file_name.ends_with(".old") || file_name.ends_with(".exe.old") {
                        info!("Menghapus file sisa update lama: {:?}", path);
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
    }
}

pub async fn run_in_app_update(
    progress: Arc<UpdateProgress>,
    download_url: String,
) -> Result<(), String> {
    info!("Memulai proses in-app self update dari URL: {}", download_url);
    progress.set_downloading(0, 0, "Menghubungi server rilis GitHub...").await;

    let current_exe = std::env::current_exe().map_err(|e| format!("Gagal mendeteksi path executable saat ini: {}", e))?;
    let exe_dir = current_exe.parent().unwrap_or_else(|| Path::new("."));
    let exe_filename = current_exe
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("rusterm.exe");

    let temp_exe = exe_dir.join(format!("{}.new", exe_filename));
    let old_exe = exe_dir.join(format!("{}.old", exe_filename));

    // 1. Download binary with streaming chunks
    let client = reqwest::Client::builder()
        .user_agent(concat!("rusterm-updater/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("Gagal membuat HTTP client: {}", e))?;

    let mut resp = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| format!("Gagal mengunduh berkas update: {}", e))?;

    if !resp.status().is_success() {
        let err_msg = format!("Server rilis mengembalikan kode status {}", resp.status());
        progress.set_error(&err_msg).await;
        return Err(err_msg);
    }

    let total_bytes = resp.content_length().unwrap_or(0);
    let mut downloaded_bytes: u64 = 0;
    let mut downloaded_data = Vec::with_capacity(total_bytes as usize);

    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("Error membaca stream data: {}", e))? {
        downloaded_bytes += chunk.len() as u64;
        downloaded_data.extend_from_slice(&chunk);

        let msg = if total_bytes > 0 {
            format!(
                "Mengunduh... {:.1} MB / {:.1} MB",
                downloaded_bytes as f64 / 1_048_576.0,
                total_bytes as f64 / 1_048_576.0
            )
        } else {
            format!("Mengunduh... {:.1} MB", downloaded_bytes as f64 / 1_048_576.0)
        };

        progress.set_downloading(downloaded_bytes, total_bytes, &msg).await;
    }

    // 2. Validate binary integrity
    progress.set_applying("Memvalidasi integritas berkas unduhan...").await;
    if let Err(err) = validate_executable_bytes(&downloaded_data) {
        let msg = format!("Validasi binary gagal: {}", err);
        progress.set_error(&msg).await;
        return Err(msg);
    }

    // 3. Write new binary to temp file
    if let Err(e) = std::fs::write(&temp_exe, &downloaded_data) {
        let msg = format!("Gagal menulis berkas sementara: {}", e);
        progress.set_error(&msg).await;
        return Err(msg);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&temp_exe, std::fs::Permissions::from_mode(0o755));
    }

    // 4. Kill other lingering background instances
    progress
        .set_applying("Menutup proses latar belakang lama & menerapkan pembaruan...")
        .await;
    kill_other_instances();

    // 5. Windows rename trick
    if old_exe.exists() {
        let _ = std::fs::remove_file(&old_exe);
    }

    if let Err(e) = std::fs::rename(&current_exe, &old_exe) {
        let msg = format!("Gagal memindahkan executable lama: {}", e);
        progress.set_error(&msg).await;
        let _ = std::fs::remove_file(&temp_exe);
        return Err(msg);
    }

    if let Err(e) = std::fs::rename(&temp_exe, &current_exe) {
        // Rollback
        let _ = std::fs::rename(&old_exe, &current_exe);
        let msg = format!("Gagal menamai executable baru: {}", e);
        progress.set_error(&msg).await;
        return Err(msg);
    }

    // Also update companion binary if exists (e.g. termdeck.exe alongside rusterm.exe)
    let alt_name = if exe_filename.eq_ignore_ascii_case("rusterm.exe") {
        "termdeck.exe"
    } else {
        "rusterm.exe"
    };
    let alt_exe = exe_dir.join(alt_name);
    if alt_exe.exists() {
        let _ = std::fs::copy(&current_exe, &alt_exe);
    }

    progress
        .set_completed("Pembaruan berhasil! Memulai ulang aplikasi RusTerm...")
        .await;

    info!("Pembaruan binary sukses diterapkan. Mempersiapkan restart aplikasi...");

    // Spawn detached task to restart after brief delay
    let exe_to_launch = current_exe.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        info!("Meluncurkan binary baru: {:?}", exe_to_launch);
        let _ = std::process::Command::new(&exe_to_launch).spawn();
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        std::process::exit(0);
    });

    Ok(())
}
