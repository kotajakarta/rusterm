# XTerm-Rust Extensions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Mengimplementasikan secara terstruktur seluruh modul pendukung TermDeck ke dalam backend Rust (`xterm-rust`): (1) Files & SFTP Explorer API, (2) Docker/Podman Containers API, dan (3) Cloudflare Tunnels & KV Sync API.

**Architecture:** Memanfaatkan runtime async native Rust (Axum, Tokio, Russh) dengan connection pool SSH yang sudah ada. Mengintegrasikan stdin/stdout streaming untuk remote python agent pada modul Files, remote CLI execution untuk Containers, dan HTTP client `reqwest` untuk Cloudflare API v4 & KV synchronization. Seluruh state persistensi disimpan pada SQLite (`xterm.sqlite`).

**Tech Stack:** Rust 1.82+, Axum 0.7, Tokio 1.40, Russh 0.45, Rusqlite 0.32, Reqwest 0.12, React 18 / Vite.

**Spec:** Mengacu pada kontrak API di `frontend/src/api.ts` dan implementasi referensi di `../xterm/backend/app/` (`files_api.py`, `containers_api.py`, `cloudflare_api.py`, `files_agent.py`).

## Global Constraints

- Backend harus tetap berkinerja tinggi, zero-leak credential (kredensial SSH selalu didekripsi hanya di memori sesi aktif via KEK).
- Tidak boleh memotong kode dengan placeholder (`// ...`).
- Kode Rust harus lulus `cargo check`, `cargo test`, dan `cargo clippy` tanpa error.
- Frontend React tests (`npm test`) harus tetap 100% passing.
- Semua endpoint API memvalidasi header session `X-Session` atau `X-Token` sesuai standar keamanan TermDeck.

## Review Focus

1. **Path Traversal & Protected Paths pada Files API**: Mencegah manipulasi direktori sensitif sistem (`/`, `/bin`, `/etc`, dll.) dan path non-absolut.
2. **Resource Leaks pada SSH Channels**: Memastikan channel SSH dan memory buffer selalu di-close/dropped dengan benar saat transfer streaming atau error.
3. **Command Injection pada Containers API**: Parameter container ID, images, dan options harus disanitasi dan di-escape sebelum dieksekusi.
4. **Cloudflare Token Exposure**: Token API Cloudflare di database harus dienkripsi dengan DEK/KEK atau diproteksi dan tidak dibocorkan di endpoint non-admin.
5. **Memory Safety saat File Upload/Download**: Menggunakan streaming chunk buffer (misal 256KB), bukan membaca file besar secara utuh ke dalam RAM.

---

## Phase 1: Cockpit Files & SFTP Explorer API (`/api/files/*`)

### Task 1.1: Database Schema & Models untuk File Remotes & Bookmarks
**Files:**
- Modify: `backend/src/models/db.rs`
- Create: `backend/src/models/files.rs`
- Modify: `backend/src/models/mod.rs`
- Test: `backend/tests/api_test.rs`

- [ ] Step 1: Buat struct model `FileRemote`, `FileRemoteInput`, `FileEntry`, `FileActionBody` di `models/files.rs`.
- [ ] Step 2: Tambahkan tabel `file_remotes` dan `file_bookmarks` di `models/db.rs` pada skema inisialisasi SQLite.
- [ ] Step 3: Implementasikan CRUD `file_remotes` dan `file_bookmarks` di `DbStore`.
- [ ] Step 4: Tulis unit test untuk database file remotes & bookmarks di `tests/api_test.rs`.
- [ ] Step 5: Jalankan `cargo test` dan pastikan lulus.
- [ ] Step 6: Commit perubahan git.

### Task 1.2: SSH Pool Stdin Support & Agent Runner
**Files:**
- Modify: `backend/src/services/ssh_pool.rs`
- Create: `backend/src/files_agent.py` (copy dari `../xterm/backend/app/files_agent.py`)
- Test: `backend/tests/ssh_pool_test.rs`

- [ ] Step 1: Salin script `files_agent.py` ke dalam `backend/src/files_agent.py`.
- [ ] Step 2: Tambahkan fungsi `run_command_with_stdin` pada `SshPool` yang membuka session channel, mengirim payload ke `channel.data()` lalu `channel.eof()`, dan mengumpulkan output JSON.
- [ ] Step 3: Implementasikan helper `run_agent(pool, creds, op, params)` untuk eksekusi python agent di node remote.
- [ ] Step 4: Tambahkan test serialization/runner di `tests/ssh_pool_test.rs`.
- [ ] Step 5: Jalankan `cargo test` dan pastikan lulus.
- [ ] Step 6: Commit perubahan git.

### Task 1.3: Implementasi Controller Files API
**Files:**
- Create: `backend/src/controllers/files.rs`
- Modify: `backend/src/controllers/mod.rs`
- Modify: `backend/src/main.rs`
- Test: `backend/tests/files_test.rs`

- [ ] Step 1: Buat handler untuk endpoint file manager:
  - `GET /api/files/:server_id/list` (listing direktori, metadata, file permissions)
  - `GET /api/files/:server_id/principals` (daftar user & group remote)
  - `GET /api/files/:server_id/read` (baca teks file untuk editor)
  - `POST /api/files/:server_id/action` (mkdir, rename, chmod, delete, extract)
  - `POST /api/files/:server_id/upload` (multipart streaming upload)
  - `GET /api/files/:server_id/download` & `GET /api/files/:server_id/view` (file streaming download dengan content-type sesuai ekstensi)
  - `GET /api/files/:server_id/thumb` (thumbnail preview)
  - `GET/POST/PUT/DELETE /api/files/remotes` (manajemen konfigurasi remote sync)
  - `GET/POST /api/files/bookmarks` (file path bookmarks)
- [ ] Step 2: Hubungkan router `/api/files` di `main.rs`.
- [ ] Step 3: Tulis integration test `tests/files_test.rs` untuk memverifikasi endpoint files.
- [ ] Step 4: Jalankan `cargo test` dan pastikan seluruh test lulus.
- [ ] Step 5: Commit perubahan git.

---

## Phase 2: Containers API (`/api/containers/*`)

### Task 2.1: Model & Schema Docker/Podman
**Files:**
- Create: `backend/src/models/containers.rs`
- Modify: `backend/src/models/mod.rs`

- [ ] Step 1: Definisikan struct `ContainerEngineInfo`, `ContainerItem`, `ContainerActionBody`, `ContainerLogsOut`, `ImageItem`, `ContainerRunBody`.
- [ ] Step 2: Implementasikan parser JSON output untuk `docker ps --format '{{json .}}'` dan `podman ps --format json`.
- [ ] Step 3: Tambahkan unit test parsing container items di `tests/containers_test.rs`.
- [ ] Step 4: Jalankan `cargo test` dan pastikan lulus.
- [ ] Step 5: Commit perubahan git.

### Task 2.2: Implementasi Controller Containers API
**Files:**
- Create: `backend/src/controllers/containers.rs`
- Modify: `backend/src/controllers/mod.rs`
- Modify: `backend/src/main.rs`
- Test: `backend/tests/containers_test.rs`

- [ ] Step 1: Implementasikan handler:
  - `GET /api/containers/engine`: Deteksi apakah Docker atau Podman terpasang via command `docker info` / `podman info`.
  - `GET /api/containers/list`: List kontainer (running & stopped).
  - `POST /api/containers/action`: Eksekusi aksi (`start`, `stop`, `restart`, `pause`, `unpause`, `delete`).
  - `GET /api/containers/logs`: Ambil logs kontainer dengan batas tail.
  - `GET /api/containers/images`: List image kontainer lokal.
  - `POST /api/containers/images/pull`: Tarik image dari registry.
  - `DELETE /api/containers/images`: Hapus image kontainer.
  - `POST /api/containers/run`: Jalankan kontainer baru dengan konfigurasi port, volume, dan environment variables.
- [ ] Step 2: Daftarkan router `/api/containers` di `main.rs`.
- [ ] Step 3: Jalankan `cargo test` dan verifikasi semua skenario handler.
- [ ] Step 4: Commit perubahan git.

---

## Phase 3: Cloudflare Sync & Tunnels (`/api/cf/*`)

### Task 3.1: Tambah Dependensi & Skema Database Cloudflare
**Files:**
- Modify: `backend/Cargo.toml`
- Modify: `backend/src/models/db.rs`
- Create: `backend/src/models/cloudflare.rs`
- Modify: `backend/src/models/mod.rs`

- [ ] Step 1: Tambahkan `reqwest = { version = "0.12", features = ["json"] }` ke `backend/Cargo.toml`.
- [ ] Step 2: Buat tabel `cf_accounts` dan `cf_sync_meta` di `models/db.rs`.
- [ ] Step 3: Buat data model dan DTO Cloudflare di `models/cloudflare.rs`.
- [ ] Step 4: Jalankan `cargo check` untuk memverifikasi dependensi baru.
- [ ] Step 5: Commit perubahan git.

### Task 3.2: Implementasi Service Client Cloudflare API v4 & KV Sync
**Files:**
- Create: `backend/src/services/cloudflare.rs`
- Modify: `backend/src/services/mod.rs`

- [ ] Step 1: Buat HTTP client wrapper Cloudflare v4 untuk verify token, list accounts, tunnels, zones, dan DNS CNAME records.
- [ ] Step 2: Implementasikan KV storage backup push, pull, dan diff calculation yang kompatibel dengan format enkripsi TermDeck.
- [ ] Step 3: Tulis unit test untuk enkripsi snapshot KV dan parsing response Cloudflare.
- [ ] Step 4: Jalankan `cargo test`.
- [ ] Step 5: Commit perubahan git.

### Task 3.3: Implementasi Controller Cloudflare API
**Files:**
- Create: `backend/src/controllers/cloudflare.rs`
- Modify: `backend/src/controllers/mod.rs`
- Modify: `backend/src/main.rs`
- Test: `backend/tests/cloudflare_test.rs`

- [ ] Step 1: Implementasikan handler:
  - CRUD Accounts: `GET /api/cf/accounts`, `POST /api/cf/accounts`, `POST /api/cf/accounts/verify`, `PUT /api/cf/accounts/:id`, `DELETE /api/cf/accounts/:id`.
  - Tunnels & Routes: `GET /api/cf/accounts/:id/tunnels`, `GET /api/cf/accounts/:id/zones`, `GET/POST/DELETE /api/cf/accounts/:id/tunnels/:tunnel_id/routes`, `GET /api/cf/accounts/:id/tunnels/:tunnel_id/token`.
  - KV Sync: `GET /api/cf/sync/status`, `POST /api/cf/sync/sync-now`, `POST /api/cf/sync/push`, `POST /api/cf/sync/pull`.
- [ ] Step 2: Daftarkan router `/api/cf` di `main.rs`.
- [ ] Step 3: Jalankan `cargo test`.
- [ ] Step 4: Commit perubahan git.

---

## Phase 4: Final Verification & Release Build

- [ ] Step 1: Jalankan semua backend unit & integration tests (`cargo test`).
- [ ] Step 2: Jalankan build frontend (`npm run build`) dan frontend test suite (`npm test`).
- [ ] Step 3: Build release binary `cargo build --release`.
- [ ] Step 4: Uji live instance `./run.sh --port 9098 --no-browser` dan verifikasi semua endpoint baru merespons dengan benar.
- [ ] Step 5: Final git commit & audit review keamanan.
