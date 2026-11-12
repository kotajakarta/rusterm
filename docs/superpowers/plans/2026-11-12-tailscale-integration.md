# Tailscale REST API Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Menambahkan integrasi Tailscale REST API ke RusTerm untuk mengelola akun API token, memantau status semua mesin/devices di Tailnet, dan mengonfigurasi pengaturan mesin (otorisasi, routing subnet/exit node, key expiry, tags, penghapusan mesin).

**Architecture:** Modul backend Rust baru (`models/tailscale.rs`, `services/tailscale.rs`, `controllers/tailscale.rs`, dan penambahan tabel di `models/db.rs`) yang mengonsumsi Tailscale REST API v2 dengan autentikasi Bearer token, dipadukan dengan modul frontend React (`TailscalePanel.tsx`, `TailscaleAccountModal.tsx`, `TailscaleDeviceModal.tsx`).

**Tech Stack:** Rust (Axum, Tokio, Reqwest, Rusqlite), React 18, TypeScript, TailwindCSS.

**Spec:** `docs/superpowers/specs/2026-11-12-tailscale-integration-design.md`

## Global Constraints
- Token API Tailscale tidak boleh dikembalikan dalam bentuk plaintext (harus dimask `••••` pada response JSON).
- Semua endpoint `/api/tailscale/*` wajib memverifikasi session PIN via `X-Session` header.
- Endpoint eksternal Tailscale menggunakan `https://api.tailscale.com/api/v2`.
- Kode TypeScript harus bebas dari tipe `any`.
- Seluruh query SQLite wajib menggunakan prepared statements (`params![]`).

## Review Focus
1. Request tanpa header `X-Session` atau dengan session tidak valid mengembalikan `401 Unauthorized`.
2. Token Tailscale tersimpan aman di database dan hasil GET akun selalu menghasilkan token termaskir (`token_masked`).
3. Payload input dari klien (routes, tags, boolean flags) divalidasi sebelum dikirimkan ke Tailscale API.
4. Error dari Tailscale API (misal token kadaluarsa, device tidak ditemukan, permission denied) ditangkap dan dikembalikan sebagai JSON error terstruktur tanpa membocorkan token di pesan error.
5. Menghapus akun Tailscale di RusTerm tidak menghapus mesin di Tailscale asli, sedangkan perintah hapus mesin memiliki konfirmasi ketat.

---

### Task 1: Database Schema & CRUD Akun Tailscale

**Files:**
- Modify: `backend/src/models/db.rs`
- Modify: `backend/src/models/mod.rs`
- Create: `backend/src/models/tailscale.rs`
- Test: `backend/tests/tailscale_db_test.rs`

**Interfaces:**
- Consumes: `rusqlite`, `DbStore`
- Produces: `TailscaleAccount`, `TailscaleAccountIn`, `TailscaleAccountUpdateIn`, `DbStore::list_tailscale_accounts`, `DbStore::get_tailscale_account`, `DbStore::create_tailscale_account`, `DbStore::update_tailscale_account`, `DbStore::delete_tailscale_account`

- [ ] **Step 1: Write the failing test**
Tulis unit test di `backend/tests/tailscale_db_test.rs` yang menguji inisialisasi tabel `tailscale_accounts`, pembuatan akun baru, pembacaan dengan masking token, pembaruan akun, dan penghapusan akun.

- [ ] **Step 2: Run test to verify it fails**
Jalankan: `cargo test --manifest-path backend/Cargo.toml --test tailscale_db_test`
Ekspektasi: FAIL karena struct dan method belum didefinisikan.

- [ ] **Step 3: Implement models and database CRUD**
1. Buat `backend/src/models/tailscale.rs` mendefinisikan struct `TailscaleAccount`, `TailscaleAccountIn`, `TailscaleAccountUpdateIn`, `TailscaleDevice`, `TailscaleDeviceRoutes`, `SetRoutesIn`, `SetAuthorizedIn`, `SetKeyExpiryIn`, `SetTagsIn`.
2. Daftarkan di `backend/src/models/mod.rs`.
3. Tambahkan migrasi tabel `tailscale_accounts` di `backend/src/models/db.rs` dan implementasikan method CRUD (`list_tailscale_accounts`, `get_tailscale_account`, `create_tailscale_account`, `update_tailscale_account`, `delete_tailscale_account`).

- [ ] **Step 4: Run test to verify it passes**
Jalankan: `cargo test --manifest-path backend/Cargo.toml --test tailscale_db_test`
Ekspektasi: PASS.

- [ ] **Step 5: Commit**
`git add backend/src/models/ backend/tests/tailscale_db_test.rs && git commit -m "feat(tailscale): add db schema and models for tailscale accounts"`

---

### Task 2: Tailscale Client Service

**Files:**
- Create: `backend/src/services/tailscale.rs`
- Modify: `backend/src/services/mod.rs`
- Modify: `backend/src/controllers/servers.rs` (AppState)
- Test: `backend/tests/tailscale_service_test.rs`

**Interfaces:**
- Consumes: `reqwest::Client`, `TailscaleDevice`, `TailscaleDeviceRoutes`
- Produces: `TailscaleClient` dengan method:
  - `verify_token(&self, token: &str, tailnet: &str) -> Result<bool, String>`
  - `list_devices(&self, token: &str, tailnet: &str) -> Result<Vec<TailscaleDevice>, String>`
  - `get_device(&self, token: &str, device_id: &str) -> Result<TailscaleDevice, String>`
  - `get_device_routes(&self, token: &str, device_id: &str) -> Result<TailscaleDeviceRoutes, String>`
  - `set_device_routes(&self, token: &str, device_id: &str, routes: &[String]) -> Result<TailscaleDeviceRoutes, String>`
  - `set_device_authorized(&self, token: &str, device_id: &str, authorized: bool) -> Result<(), String>`
  - `set_device_key_expiry(&self, token: &str, device_id: &str, disabled: bool) -> Result<(), String>`
  - `set_device_tags(&self, token: &str, device_id: &str, tags: &[String]) -> Result<(), String>`
  - `delete_device(&self, token: &str, device_id: &str) -> Result<(), String>`

- [ ] **Step 1: Write the failing test**
Tulis test di `backend/tests/tailscale_service_test.rs` yang menguji deserialisasi response Tailscale API (mock devices, online status parsing, routes parsing).

- [ ] **Step 2: Run test to verify it fails**
Jalankan: `cargo test --manifest-path backend/Cargo.toml --test tailscale_service_test`
Ekspektasi: FAIL karena `TailscaleClient` belum ada.

- [ ] **Step 3: Implement TailscaleClient**
1. Buat `backend/src/services/tailscale.rs` dengan `TailscaleClient` mengimplementasikan fungsi-fungsi HTTP request ke Tailscale REST API v2 (`https://api.tailscale.com/api/v2`).
2. Pasang authorization header `Bearer <token>` dan user-agent `RusTerm-Tailscale/1.0`.
3. Daftarkan di `backend/src/services/mod.rs` dan pasang di `AppState` (`backend/src/controllers/servers.rs`).

- [ ] **Step 4: Run test to verify it passes**
Jalankan: `cargo test --manifest-path backend/Cargo.toml --test tailscale_service_test`
Ekspektasi: PASS.

- [ ] **Step 5: Commit**
`git add backend/src/services/ backend/src/controllers/servers.rs backend/tests/tailscale_service_test.rs && git commit -m "feat(tailscale): add tailscale client service"`

---

### Task 3: Backend Controller & Router

**Files:**
- Create: `backend/src/controllers/tailscale.rs`
- Modify: `backend/src/controllers/mod.rs`
- Modify: `backend/src/main.rs`
- Test: `backend/tests/tailscale_api_test.rs`

**Interfaces:**
- Consumes: `AppState`, `TailscaleClient`, `DbStore`, `Vault`
- Produces: `tailscale::router(state: AppState) -> Router`

- [ ] **Step 1: Write the failing test**
Tulis integrasi test di `backend/tests/tailscale_api_test.rs` meniru `backend/tests/cloudflare_test.rs` untuk memverifikasi autentikasi endpoint (`401 Unauthorized` tanpa session) dan fungsionalitas CRUD akun via HTTP.

- [ ] **Step 2: Run test to verify it fails**
Jalankan: `cargo test --manifest-path backend/Cargo.toml --test tailscale_api_test`
Ekspektasi: FAIL.

- [ ] **Step 3: Implement controller and mount router**
1. Buat `backend/src/controllers/tailscale.rs` dengan handler untuk:
   - `/tailscale/accounts` (GET, POST)
   - `/tailscale/accounts/verify` (POST)
   - `/tailscale/accounts/:id` (PUT, DELETE)
   - `/tailscale/accounts/:id/devices` (GET)
   - `/tailscale/accounts/:id/devices/:device_id` (GET, DELETE)
   - `/tailscale/accounts/:id/devices/:device_id/routes` (GET, POST)
   - `/tailscale/accounts/:id/devices/:device_id/authorized` (POST)
   - `/tailscale/accounts/:id/devices/:device_id/key` (POST)
   - `/tailscale/accounts/:id/devices/:device_id/tags` (POST)
2. Daftarkan di `backend/src/controllers/mod.rs` dan kaitkan router di `backend/src/main.rs`.

- [ ] **Step 4: Run test to verify it passes**
Jalankan: `cargo test --manifest-path backend/Cargo.toml --test tailscale_api_test`
Ekspektasi: PASS.

- [ ] **Step 5: Commit**
`git add backend/src/controllers/ backend/src/main.rs backend/tests/tailscale_api_test.rs && git commit -m "feat(tailscale): add tailscale api endpoints and router"`

---

### Task 4: Frontend Types, API Client & State

**Files:**
- Modify: `frontend/src/types.ts`
- Modify: `frontend/src/api.ts`

**Interfaces:**
- Consumes: Backend `/api/tailscale/*`
- Produces: Types (`TailscaleAccount`, `TailscaleDevice`, `TailscaleDeviceRoutes`, `TailscaleAccountIn`, `TailscaleAccountUpdateIn`, dsb) dan fungsi-fungsi `api.tailscale*` di `frontend/src/api.ts`.

- [ ] **Step 1: Define TypeScript interfaces in types.ts**
Tambahkan tipe-tipe data Tailscale lengkap tanpa `any`.

- [ ] **Step 2: Add API methods in api.ts**
Tambahkan fungsi pemanggil API: `tailscaleListAccounts`, `tailscaleCreateAccount`, `tailscaleUpdateAccount`, `tailscaleDeleteAccount`, `tailscaleVerifyAccount`, `tailscaleListDevices`, `tailscaleGetDevice`, `tailscaleGetDeviceRoutes`, `tailscaleSetDeviceRoutes`, `tailscaleSetDeviceAuthorized`, `tailscaleSetDeviceKeyExpiry`, `tailscaleSetDeviceTags`, `tailscaleDeleteDevice`.

- [ ] **Step 3: Verify TypeScript compilation**
Jalankan verifikasi syntax dan tipe pada frontend.

- [ ] **Step 4: Commit**
`git add frontend/src/types.ts frontend/src/api.ts && git commit -m "feat(tailscale): add frontend types and api client methods"`

---

### Task 5: Frontend UI - TailscalePanel, TailscaleAccountModal & TailscaleDeviceModal

**Files:**
- Create: `frontend/src/components/TailscalePanel.tsx`
- Create: `frontend/src/components/TailscaleAccountModal.tsx`
- Create: `frontend/src/components/TailscaleDeviceModal.tsx`
- Modify: `frontend/src/components/Sidebar.tsx` atau `frontend/src/App.tsx`

**Interfaces:**
- Consumes: `frontend/src/api.ts`, `frontend/src/types.ts`
- Produces: UI panel Tailscale di sidebar dan modal-modal manajemen akun serta pengaturan mesin.

- [ ] **Step 1: Create TailscaleAccountModal.tsx**
Form untuk menambah akun (Nama, Tailnet [default `-`], API Access Token), verifikasi token sebelum simpan, update akun, dan hapus akun.

- [ ] **Step 2: Create TailscaleDeviceModal.tsx**
Modal interaktif untuk memeriksa semua status & setting mesin:
- Tampilan detail: ID, Hostname, OS, Addresses, User, Last Seen, Created, Key Expiration.
- Kontrol Setting: Toggle Authorized, Toggle Key Expiry Disabled, Checklist Advertised & Enabled Routes (subnet / exit node), Input Tags, dan Tombol Hapus Mesin dari Tailnet.

- [ ] **Step 3: Create TailscalePanel.tsx and integrate into Sidebar/App**
Panel akordeon di sidebar menampilkan dropdown pilihan akun Tailscale, tombol refresh, tombol kelola akun, dan daftar perangkat Tailscale (dengan lampu status hijau/abu-abu, hostname, IP 100.x.y.z + tombol salin, badge OS & routes, tombol buka modal setting, dan tombol opsi buat koneksi SSH cepat). Pasang di sidebar RusTerm.

- [ ] **Step 4: Verify integration and styling**
Pastikan komponen terpasang dengan baik, layout responsif dan konsisten dengan tema RusTerm.

- [ ] **Step 5: Commit**
`git add frontend/src/components/ && git commit -m "feat(tailscale): add tailscale panel, account modal, and device settings modal"`

---

### Task 6: Full Verification & Security Audit

**Files:**
- All touched files
- Run: `cargo test --manifest-path backend/Cargo.toml`
- Security Audit Table verification

- [ ] **Step 1: Run all tests in backend**
`cargo test --manifest-path backend/Cargo.toml`
Pastikan 100% test lulus.

- [ ] **Step 2: Security and safety check**
Lakukan audit otorisasi, proteksi injeksi, masking rahasia/token, dan dependensi.

- [ ] **Step 3: Final commit**
`git commit -m "feat(tailscale): complete tailscale rest api integration"`
