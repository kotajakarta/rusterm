# Tailscale REST API Integration Design

## Intent & Why
Pengguna ingin menghubungkan akun Tailscale ke RusTerm (XTerm-Rust) menggunakan Tailscale API Access Token (`tskey-api-...`). Integrasi ini memungkinkan pengguna untuk memantau status mesin/perangkat (devices) di Tailnet mereka secara real-time dan mengelola konfigurasi/setting setiap mesin (seperti otorisasi perangkat, pembaruan key expiry, routing subnet/exit node, pemberian tags, dan pelepasan mesin dari Tailnet) langsung dari antarmuka RusTerm.

## Goals
1. **Manajemen Akun Tailscale**:
   - Mendukung penyimpanan token API (`tskey-api-...`) dan Tailnet ID/name (default `-`) secara aman di SQLite dengan enkripsi/masking.
   - Verifikasi token langsung ke endpoint Tailscale API sebelum disimpan atau diperbarui.
   - CRUD akun Tailscale (tambah, edit nama/token/tailnet, hapus).
2. **Monitoring Status Mesin (Devices)**:
   - Menampilkan daftar semua mesin di Tailnet (hostname, IP Tailscale 100.x.y.z, OS, versi client, update available, status online/offline berdasarkan `lastSeen` dan konektivitas, kepemilikan user, dan tanggal dibuat/kadaluarsa).
   - Indikator status visual (online/offline, exit node active, subnet routes active, expiry warnings).
   - Tombol cepat salin IP Tailscale dan opsi cepat hubungkan ke SSH (prefill atau koneksi langsung).
3. **Manajemen Setting Mesin (Device Settings)**:
   - **Authorized**: Mengubah status otorisasi mesin (approve / unapprove di Tailnet).
   - **Key Expiry**: Mengaktifkan / menonaktifkan expiry key (`keyExpiryDisabled`).
   - **Routes**: Melihat subnet routes & exit node routes yang diiklankan (advertised routes), serta mengaktifkan/menonaktifkan routes yang disetujui (enabled routes).
   - **Tags**: Mengelola daftar tag (misal `tag:server`, `tag:prod`).
   - **Delete Machine**: Menghapus mesin dari Tailnet dengan konfirmasi aman.

## Non-Goals & Anti-Goals
- **Non-Goals**: Menjalankan binary CLI Tailscale lokal di host atau instalasi daemon Tailscale otomatis pada server remote. Pengelolaan murni menggunakan Tailscale REST API v2 resmi (`https://api.tailscale.com/api/v2`).
- **Anti-Goals**:
  - Menyimpan token API dalam bentuk plaintext tanpa enkripsi/masking di response API.
  - Mengizinkan request tidak terotentikasi (semua endpoint dilindungi session vault PIN).

## Constraints
- Backend: Rust Axum, Tokio, Reqwest, Rusqlite.
- Frontend: React 18, TypeScript, Vite, TailwindCSS/CSS kustom RusTerm.
- Zero plaintext credentials: token di database tersimpan aman, token yang dikembalikan ke browser dimask (`tskey-api-••••...`).
- Arsitektur mengikuti modul Cloudflare yang sudah terbukti andal di RusTerm.

## Approaches
1. **Pendekatan A: Pola Modular Controller + Service + DB Mirip Cloudflare (Dipilih)**
   - Menambahkan tabel `tailscale_accounts` di SQLite.
   - Membuat `TailscaleClient` di `backend/src/services/tailscale.rs` menggunakan `reqwest`.
   - Membuat `tailscale::router` di `backend/src/controllers/tailscale.rs` dengan validasi otentikasi session.
   - Menambahkan komponen UI `TailscalePanel`, `TailscaleAccountModal`, dan `TailscaleDeviceModal` di frontend.
   - *Kelebihan*: Konsisten dengan struktur Cloudflare, terisolasi, aman, mudah diuji secara unit & integrasi.
2. **Pendekatan B: Eksekusi Tailscale CLI Shell Wrapper**
   - Menjalankan `tailscale` binary di mesin lokal via `std::process::Command`.
   - *Kelemahan*: Bergantung pada daemon tailscale lokal yang harus terpasang di host, tidak bisa mengelola mesin di remote tailnet jika host tidak terhubung ke tailnet tersebut, rawan shell injection.
   - *Alasan ditolak*: Pengguna secara eksplisit meminta REST API, dan REST API lebih fleksibel serta aman.

## Arsitektur & Endpoint API
Base URL Tailscale: `https://api.tailscale.com/api/v2`

### Endpoint RusTerm Backend:
- `GET /api/tailscale/accounts`: Daftar akun Tailscale yang tersimpan (token dimask).
- `POST /api/tailscale/accounts`: Tambah akun baru.
- `POST /api/tailscale/accounts/verify`: Uji validitas token & tailnet ke Tailscale API.
- `PUT /api/tailscale/accounts/:id`: Update nama, token, atau tailnet.
- `DELETE /api/tailscale/accounts/:id`: Hapus akun Tailscale.
- `GET /api/tailscale/accounts/:id/devices`: Ambil daftar mesin beserta statusnya.
- `GET /api/tailscale/accounts/:id/devices/:device_id`: Detail satu mesin.
- `GET /api/tailscale/accounts/:id/devices/:device_id/routes`: Ambil advertised & enabled routes.
- `POST /api/tailscale/accounts/:id/devices/:device_id/routes`: Set enabled routes.
- `POST /api/tailscale/accounts/:id/devices/:device_id/authorized`: Set authorized (true/false).
- `POST /api/tailscale/accounts/:id/devices/:device_id/key`: Set key expiry disabled (true/false).
- `POST /api/tailscale/accounts/:id/devices/:device_id/tags`: Set tags.
- `DELETE /api/tailscale/accounts/:id/devices/:device_id`: Hapus device dari tailnet.
