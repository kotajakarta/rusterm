# Design Document: Standalone Windows Executable (.exe) for TermDeck

**Date:** 2026-11-10  
**Status:** In Review  
**Target Platform:** Windows 10/11 x86_64  

---

## 1. Intent and The Why

Pengguna menginginkan file executable Windows (`.exe`) mandiri (*standalone executable*) untuk TermDeck Rust. Saat ini TermDeck Rust berjalan di Linux/WSL dan menyajikan file statis frontend dari direktori `frontend/dist` di sistem file. 

Tujuan utama dari desain ini adalah menghasilkan **satu file `.exe` portabel tunggal (`termdeck.exe`)** yang dapat diunduh atau dipindahkan ke mesin Windows mana pun dan langsung dijalankan dengan cara klik-dua-kali (*double click*), tanpa mengharuskan instalasi WSL, Python, Node.js, atau menyertakan folder aset web secara terpisah.

---

## 2. Goals, Non-Goals, and Anti-Goals

### Goals
- **Single Portable Binary:** Menghasilkan satu file executable Windows mandiri (`termdeck.exe`) yang telah menyematkan (*embed*) seluruh aset frontend (HTML, CSS, JS, font, favicon) di dalam binary menggunakan crate `rust-embed`.
- **Pure GUI Window (Tanpa Konsol):** Binary dikompilasi dengan atribut `#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]` sehingga saat diklik dua kali, tidak ada jendela Command Prompt hitam yang muncul.
- **Automatic App Window:** Saat dijalankan di Windows tanpa flag `--no-browser`, aplikasi otomatis mendeteksi browser Chromium bawaan sistem (Microsoft Edge `msedge.exe` atau Google Chrome `chrome.exe`) dan membukanya dalam mode aplikasi mandiri (`--app=http://127.0.0.1:<port>`) dengan direktori profil terisolasi (`%LOCALAPPDATA%\TermDeck\browser`).
- **Dynamic Token Ingestion:** Mekanisme proteksi sesi TermDeck (injeksi token acak 32 karakter ke dalam tag `<meta name="termdeck-token">` pada `index.html`) tetap berfungsi secara dinamis dari memori.
- **Windows Local Storage & File Logging:** Database SQLite (`xterm.sqlite`), vault (`vault.json`), dan file log (`termdeck.log`) disimpan di direktori lokal Windows (`%LOCALAPPDATA%\TermDeck\`).
- **Single-Instance Re-launch Protection:** Jika `termdeck.exe` diklik kedua kali saat instans sebelumnya sudah aktif, instans baru cukup memfokuskan/membuka kembali browser ke URL aktif lalu langsung exit secara bersih tanpa bentrok port.
- **Development Fallback:** Jika opsi `--dist-dir <path>` diberikan, server tetap memprioritaskan penyajian file dari disk lokal (mempertahankan kecepatan iterasi frontend dev).
- **Cross-Compilation:** Skrip build (`build-windows.sh`) yang dapat mengkompilasi target Windows `x86_64-pc-windows-gnu` / `x86_64-pc-windows-msvc` langsung dari lingkungan Linux/WSL.

### Non-Goals
- **Embedded Heavy Chromium Engine:** Tidak menyematkan mesin browser Chromium utuh (seperti Electron atau CEF) yang membuat ukuran binary membengkak hingga ratusan megabyte. Kita memanfaatkan Edge/Chrome bawaan yang sudah terpasang di Windows.
- **Custom Native Win32 UI:** Tidak membangun widget form atau windowing Win32 native; semua antarmuka pengguna tetap menggunakan React frontend TermDeck yang sudah ada.
- **Support for 32-bit Windows:** Fokus pada arsitektur modern Windows 64-bit (`x86_64`).

### Anti-Goals
- Tidak memerlukan instalasi runtime tambahan (Node.js, Python, WSL, Visual Studio C++ Redistributable terpisah).
- Tidak mengunci file database di folder sementara (`%TEMP%`) yang bisa terhapus oleh sistem pembersih disk.

---

## 3. Approaches Considered

### Pendekatan 1: Single Portable `.exe` (Embedded Frontend + Auto App-Window) — *Terpilih*
- **Deskripsi:** Seluruh bundle `frontend/dist` dikompilasi langsung ke dalam binary Rust via `rust-embed`. Saat dijalankan, binary otomatis mencari Edge/Chrome dan membuka jendela aplikasi mandiri.
- **Trade-off:** Menambah ukuran binary sekitar 1-2 MB (total file `.exe` ~15 MB), tetapi memberikan pengalaman pengguna terbaik karena hanya berupa satu file tunggal yang mandiri dan portabel.

### Pendekatan 2: Portable Folder (`termdeck.exe` + Folder `dist/`)
- **Deskripsi:** Binary `.exe` didistribusikan dalam bentuk file ZIP bersama folder `dist/`.
- **Trade-off:** Ukuran `.exe` lebih kecil, namun bukan satu file mandiri. Pengguna bisa salah menaruh atau menghapus folder `dist/` sehingga aplikasi gagal memuat UI.

### Pendekatan 3: Native Tray Application (Embedded Frontend + Win32 Event Loop)
- **Deskripsi:** Menambahkan system tray icon di pojok taskbar Windows dekat jam dengan menu klik kanan.
- **Trade-off:** Lebih kompleks dan membutuhkan penanganan event loop Win32 native di backend.

**Pilihan:** Pendekatan 1 dipilih karena memenuhi kebutuhan pengguna secara tepat, sederhana, dan menghasilkan satu file executable portabel murni.

---

## 4. Technical Architecture & Implementation Details

```
+-------------------------------------------------------------+
|                        termdeck.exe                         |
|                                                             |
|  +-------------------------------------------------------+  |
|  | rust-embed: Embedded Assets (HTML, JS, CSS, Fonts)    |  |
|  +-------------------------------------------------------+  |
|                                                             |
|  +-----------------------+     +-------------------------+  |
|  | Axum HTTP & WS Server |     | Windows App Launcher    |  |
|  |  - /api/*             |     |  - Cari msedge / chrome |  |
|  |  - /ws/terminal       |     |  - Luncurkan --app      |  |
|  |  - / (injeksi token)  |     +-------------------------+  |
|  +-----------------------+                                  |
|              |                                              |
|              v                                              |
|  +-------------------------------------------------------+  |
|  | Data Store (%LOCALAPPDATA%\TermDeck\)                 |  |
|  |  - xterm.sqlite                                       |  |
|  |  - vault.json                                         |  |
|  |  - browser/ (profil Edge/Chrome terisolasi)           |  |
|  +-------------------------------------------------------+  |
+-------------------------------------------------------------+
```

### 4.1 Asset Embedding (`rust-embed`)
1. Tambahkan dependensi `rust-embed = "8.5"` ke `backend/Cargo.toml`.
2. Di `controllers/static_files.rs`:
   - Definisikan `#[derive(RustEmbed)] #[folder = "../frontend/dist"] struct Asset;`.
   - Modifikasi router static file:
     - Jika `dist_dir` ada dan folder fisiknya ada di disk, layani dari disk (dev mode).
     - Jika tidak ada di disk atau `dist_dir` adalah fallback default, layani file dari struct `Asset`.
     - Untuk route `/` dan file `index.html`: ambil konten `index.html` dari `Asset::get("index.html")`, decode ke UTF-8 string, ganti `<meta name="termdeck-token" content="">` dengan token acak yang dibuat saat startup, lalu kembalikan response `text/html; charset=utf-8` dengan header `Cache-Control: no-store`.
     - Untuk file statis lainnya: tentukan MIME type dengan `mime_guess`, sajikan dengan header caching yang tepat.

### 4.2 Windows App Launcher Service
Tambahkan modul peluncur jendela mandiri (`services/app_launcher.rs`):
- Deteksi sistem operasi:
  - Pada Windows (`#[cfg(target_os = "windows")]` atau saat runtime):
    - Cari executable browser dengan urutan prioritas:
      1. Microsoft Edge: `%ProgramFiles(x86)%\Microsoft\Edge\Application\msedge.exe` atau `%ProgramFiles%\Microsoft\Edge\Application\msedge.exe`
      2. Google Chrome: `%ProgramFiles%\Google\Chrome\Application\chrome.exe` atau `%ProgramFiles(x86)%\Google\Chrome\Application\chrome.exe` atau `%LOCALAPPDATA%\Google\Chrome\Application\chrome.exe`
      3. Brave Browser: `%ProgramFiles%\BraveSoftware\Brave-Browser\Application\brave.exe`
    - Tentukan lokasi profil: `%LOCALAPPDATA%\TermDeck\browser`.
    - Eksekusi subprocess:
      ```rust
      Command::new(browser_path)
          .arg(format!("--app=http://127.0.0.1:{}", port))
          .arg(format!("--user-data-dir={}", profile_path.display()))
          .arg("--no-first-run")
          .arg("--no-default-browser-check")
          .spawn()?;
      ```
  - Fallback jika tidak ada Chromium browser: buka URL default via perintah sistem (`start <url>`).

### 4.3 Data Directory Path Cross-Platform
- Crate `dirs::data_local_dir()` yang sudah dipakai di `main.rs`:
  - Di Windows, otomatis mengarah ke `C:\Users\<User>\AppData\Local`.
  - Mengubah path dasar default dari `.join("xterm-rust")` menjadi `.join("TermDeck")` agar konsisten dengan penamaan aplikasi desktop.

### 4.4 Toolchain & Cross-Compilation
- Menggunakan cross-compilation `x86_64-pc-windows-gnu` menggunakan `mingw-w64` GCC atau `x86_64-pc-windows-msvc` menggunakan `cargo-xwin`.
- Membuat skrip build mandiri `build-windows.sh`:
  1. Build frontend: `(cd frontend && npm run build)`.
  2. Pastikan `frontend/dist/index.html` tersedia sebelum kompilasi Rust.
  3. Jalankan `cargo build --release --target x86_64-pc-windows-gnu` (atau `--target x86_64-pc-windows-msvc`).
  4. Salin hasil binary ke `dist-release/termdeck.exe`.

### 4.5 Subsystem GUI & File Logging
- Pada file `main.rs`, tambahkan conditional inner attribute:
  ```rust
  #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
  ```
  Ini memastikan saat release build dijalankan di Windows, tidak ada console prompt hitam yang muncul.
- Karena stdout/stderr tidak terhubung ke konsol, logging dialihkan menggunakan `tracing_appender::rolling::never` ke file `%LOCALAPPDATA%\TermDeck\termdeck.log`.

### 4.6 Single-Instance Detection & Lifecycle Shutdown
1. **Single-Instance Re-launch Check:**
   - Sebelum melakukan `TcpListener::bind(port)`, aplikasi melakukan healthcheck singkat ke `http://127.0.0.1:<port>/api/auth/status`.
   - Jika instans TermDeck lain sudah aktif di port tersebut:
     - Instans baru memanggil peluncur browser untuk membuka/memfokuskan URL TermDeck.
     - Instans baru langsung exit dengan kode 0 (tidak crash dan tidak mencoba mengambil alih database).
2. **Lifecycle & Clean Shutdown:**
   - Saat meluncurkan child browser (Edge/Chrome) dengan profil khusus `--user-data-dir`, browser berjalan sebagai proses terpisah.
   - Penambahan flag `--disable-features=msStartupBoost` pada Edge untuk mencegah detasemen prematur proses browser.
   - Backend memantau proses child browser atau mendengarkan sinyal shutdown.
   - Pada saat shutdown, `vault.lock().await` dipanggil sebelum proses berakhir guna memastikan seluruh rahasia di-wipe dari memori dan file database tertutup rapi.

---

## 5. What's Left to the Builder
- Memilih dan mengkonfigurasi toolchain cross-compiler yang paling cocok di lingkungan build (`mingw-w64` vs `cargo-xwin`).
- Mengatur konfigurasi `mime_guess` untuk menangani jenis file aset statis (`.wasm`, `.woff2`, `.svg`, `.css`, `.js`).
- Menulis unit/integration test untuk memastikan `Asset::get` dan penggantian token `index.html` berfungsi 100% tanpa disk.
