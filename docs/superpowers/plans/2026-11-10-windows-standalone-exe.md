# Windows Standalone Executable (.exe) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Menghasilkan satu file executable Windows mandiri (`termdeck.exe`) yang menyematkan seluruh aset frontend React di dalam binary, menyembunyikan jendela konsol (mode GUI), otomatis meluncurkan jendela aplikasi mandiri via Edge/Chrome, dan memiliki proteksi single-instance.

**Architecture:** Menggunakan crate `rust-embed` untuk menyematkan `frontend/dist` langsung ke dalam binary Rust Axum; mengarahkan konfigurasi logging ke file `%LOCALAPPDATA%\TermDeck\termdeck.log`; menggunakan inner attribute `#![windows_subsystem = "windows"]` untuk meniadakan jendela konsol; menambahkan modul peluncur browser Edge/Chrome dengan profil terisolasi; dan menyediakan skrip cross-compilation `build-windows.sh`.

**Tech Stack:** Rust (Axum, Tokio, `rust-embed`, `mime_guess`, `tracing-appender`), React + Vite frontend, target `x86_64-pc-windows-gnu` / `x86_64-pc-windows-msvc`.

**Spec:** [`docs/superpowers/specs/2026-11-10-windows-standalone-exe-design.md`](file:///home/aithendi/webku/xterm-rust/docs/superpowers/specs/2026-11-10-windows-standalone-exe-design.md)

## Global Constraints
- Target platform: Windows 10/11 x86_64 (`x86_64-pc-windows-gnu` / `x86_64-pc-windows-msvc`).
- Standalone portable binary: seluruh aset statis frontend harus tertanam di dalam `.exe`, tanpa ketergantungan folder eksternal.
- Proteksi sesi: tag `<meta name="termdeck-token" content="...">` di `index.html` harus tetap diinjeksi secara dinamis per startup.
- Data directory: `%LOCALAPPDATA%\TermDeck\` untuk `xterm.sqlite`, `vault.json`, `termdeck.log`, dan profil browser `browser/`.
- Tidak ada jendela konsol hitam saat dijalankan via double-click pada Windows release build.

## Review Focus
1. **Penyajian aset embedded vs fallback disk:** Jika opsi `--dist-dir` diberikan, aset disk harus tetap didahulukan untuk development.
2. **Injeksi token pada embedded index.html:** Token session acak 32 char harus disisipkan ke HTML in-memory tanpa merusak struktur UTF-8.
3. **MIME type guessing pada embedded files:** File `.wasm`, `.woff2`, `.svg`, `.js`, `.css` harus memiliki Content-Type yang valid saat disajikan dari `Asset::get()`.
4. **Browser detection di Windows:** Path Edge dan Chrome harus dicari di `%ProgramFiles%`, `%ProgramFiles(x86)%`, dan `%LOCALAPPDATA%` dengan toleransi spasi nama folder.
5. **Single-instance re-launch:** Jika port 9095 sudah aktif melayani TermDeck, proses kedua harus fokus browser dan exit dengan kode 0 tanpa panik atau error bind.

---

### Task 1: Embedded Static Assets via `rust-embed`

**Files:**
- Modify: `backend/Cargo.toml`
- Modify: `backend/src/controllers/static_files.rs`
- Test: `backend/tests/static_test.rs`

**Interfaces:**
- Consumes: `StaticConfig { dist_dir: PathBuf, token: String }`
- Produces: `Asset` struct implementing `RustEmbed`, static router yang menyajikan file dari memori saat folder fisik tidak ditemukan.

- [ ] **Step 1: Write the failing test for embedded asset resolution**

Tambahkan pengujian di `backend/tests/static_test.rs`:
```rust
#[tokio::test]
async fn test_embedded_asset_serving_and_token_injection() {
    let app = static_files::router(StaticConfig {
        dist_dir: PathBuf::from("/non/existent/path"),
        token: "embedded_test_token_12345".to_string(),
    });

    let client = axum_test::TestServer::new(app).unwrap();
    let response = client.get("/").await;
    assert_eq!(response.status_code(), 200);
    let body = response.text();
    assert!(body.contains("name=\"termdeck-token\" content=\"embedded_test_token_12345\""));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /home/aithendi/webku/xterm-rust/backend && cargo test --test static_test`  
Expected: FAIL (belum ada `rust-embed` dan router gagal melayani path non-existent).

- [ ] **Step 3: Implement `Asset` with `rust-embed` in `controllers/static_files.rs`**

1. Tambahkan `rust-embed = { version = "8.5", features = ["interpolate-folder-path"] }` ke `backend/Cargo.toml`.
2. Di `controllers/static_files.rs`:
   ```rust
   use rust_embed::RustEmbed;

   #[derive(RustEmbed)]
   #[folder = "../frontend/dist"]
   pub struct EmbeddedAssets;
   ```
3. Update handler:
   - Jika file ada di `dist_dir` fisik, layani via `ServeDir` / file reader.
   - Jika tidak ada di disk, ambil dari `EmbeddedAssets::get(path)`.
   - Untuk `/` atau `index.html`: ambil dari `EmbeddedAssets::get("index.html")`, inject token ke `<meta name="termdeck-token" content="...">`, return HTML dengan `Cache-Control: no-store`.
   - Untuk aset lain: gunakan `mime_guess::from_path(path).first_or_octet_stream()` untuk set `Content-Type`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /home/aithendi/webku/xterm-rust/backend && cargo test --test static_test`  
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add backend/Cargo.toml backend/src/controllers/static_files.rs backend/tests/static_test.rs
git commit -m "feat(static): embed frontend assets using rust-embed with dynamic token injection"
```

---

### Task 2: Windows App Launcher & Subsystem Configuration

**Files:**
- Create: `backend/src/services/app_launcher.rs`
- Modify: `backend/src/services/mod.rs`
- Modify: `backend/src/main.rs`
- Test: `backend/tests/app_launcher_test.rs`

**Interfaces:**
- Consumes: `port: u16`, `base_data_dir: &Path`
- Produces: `pub fn launch_app_window(port: u16, data_dir: &Path) -> bool`

- [ ] **Step 1: Write the failing test for app launcher command construction**

Buat `backend/tests/app_launcher_test.rs`:
```rust
use xterm_backend::services::app_launcher::{build_app_args, find_browser_executable};

#[test]
fn test_build_app_args() {
    let port = 9095;
    let profile = std::path::Path::new("C:\\Users\\Test\\AppData\\Local\\TermDeck\\browser");
    let args = build_app_args(port, profile);
    
    assert!(args.contains(&"--app=http://127.0.0.1:9095".to_string()));
    assert!(args.contains(&"--no-first-run".to_string()));
    assert!(args.contains(&"--no-default-browser-check".to_string()));
    assert!(args.contains(&"--disable-features=msStartupBoost".to_string()));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /home/aithendi/webku/xterm-rust/backend && cargo test --test app_launcher_test`  
Expected: FAIL (`app_launcher` module not found).

- [ ] **Step 3: Implement `app_launcher.rs` and update `main.rs`**

1. Buat `backend/src/services/app_launcher.rs`:
   - `build_app_args(port: u16, profile_dir: &Path) -> Vec<String>`
   - `find_browser_executable() -> Option<PathBuf>` (Cari Edge di ProgramFiles / ProgramFiles(x86), Chrome di ProgramFiles / LocalAppData, Brave).
   - `launch_app_window(port: u16, data_dir: &Path) -> bool`
2. Di `backend/src/services/mod.rs`, expose `pub mod app_launcher;`.
3. Di `backend/src/main.rs`:
   - Tambahkan `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` di baris teratas.
   - Setup file logging: jika di Windows, gunakan `tracing_appender::rolling::never(&base_data_dir, "termdeck.log")`.
   - Di akhir startup (sebelum server loop), panggil `app_launcher::launch_app_window(port, &base_data_dir)` secara asynchronous jika `!no_browser`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /home/aithendi/webku/xterm-rust/backend && cargo test --test app_launcher_test`  
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add backend/src/services/app_launcher.rs backend/src/services/mod.rs backend/src/main.rs backend/tests/app_launcher_test.rs
git commit -m "feat(launcher): implement Windows Edge/Chrome app window launcher and GUI subsystem"
```

---

### Task 3: Single-Instance Detection & Graceful Lifecycle

**Files:**
- Modify: `backend/src/services/app_launcher.rs`
- Modify: `backend/src/main.rs`
- Test: `backend/tests/app_launcher_test.rs`

**Interfaces:**
- Consumes: `port: u16`
- Produces: `pub async fn check_existing_instance(port: u16) -> bool`

- [ ] **Step 1: Write test for single instance check**

Di `backend/tests/app_launcher_test.rs`:
```rust
#[tokio::test]
async fn test_check_existing_instance_non_running_port() {
    // Port 59123 bebas
    let is_running = xterm_backend::services::app_launcher::check_existing_instance(59123).await;
    assert!(!is_running);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd /home/aithendi/webku/xterm-rust/backend && cargo test --test app_launcher_test`  
Expected: FAIL (method not found).

- [ ] **Step 3: Implement `check_existing_instance` and wire into `main.rs`**

1. Di `app_launcher.rs`:
   ```rust
   pub async fn check_existing_instance(port: u16) -> bool {
       let client = reqwest::Client::builder()
           .timeout(std::time::Duration::from_millis(600))
           .build()
           .unwrap_or_default();
       let url = format!("http://127.0.0.1:{}/api/auth/status", port);
       client.get(&url).send().await.is_ok()
   }
   ```
2. Di `main.rs`:
   Sebelum inisialisasi vault & DB, panggil `check_existing_instance(port).await`.
   Jika `true`:
   - Panggil `launch_app_window(port, &base_data_dir)` untuk membuka jendela browser.
   - Return `Ok(())` langsung untuk exit secara bersih (kode 0).

- [ ] **Step 4: Run test to verify it passes**

Run: `cd /home/aithendi/webku/xterm-rust/backend && cargo test`  
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add backend/src/services/app_launcher.rs backend/src/main.rs backend/tests/app_launcher_test.rs
git commit -m "feat(lifecycle): add single-instance detection and re-launch focus handler"
```

---

### Task 4: Cross-Compilation Setup & Build Script (`build-windows.sh`)

**Files:**
- Create: `build-windows.sh`
- Test: manual execution & binary inspection via `file` / `objdump` / test run

**Interfaces:**
- Consumes: `frontend/dist/`, `backend/`
- Produces: `dist-release/termdeck.exe` (PE32+ executable for MS Windows x86-64)

- [ ] **Step 1: Write `build-windows.sh` script**

Buat skrip `build-windows.sh`:
1. Verifikasi toolchain Rust dan target Windows (`rustup target add x86_64-pc-windows-gnu` atau `cargo-xwin`).
2. Build frontend: `(cd frontend && npm run build)`.
3. Jalankan build release Windows.
4. Buat folder `dist-release/` dan salin binary ke `dist-release/termdeck.exe`.
5. Tampilkan ukuran file dan info binary (`ls -lh dist-release/termdeck.exe`).

- [ ] **Step 2: Run `build-windows.sh` to produce Windows `.exe`**

Run: `chmod +x build-windows.sh && ./build-windows.sh`  
Expected: Berhasil menghasilkan `dist-release/termdeck.exe`.

- [ ] **Step 3: Verify binary header with `file` command**

Run: `file dist-release/termdeck.exe`  
Expected: `PE32+ executable (GUI) x86-64, for MS Windows`

- [ ] **Step 4: Commit**

```bash
git add build-windows.sh
git commit -m "feat(build): add automated Windows standalone .exe build script"
```
