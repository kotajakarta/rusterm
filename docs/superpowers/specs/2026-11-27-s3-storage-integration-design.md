# S3 / R2 / MinIO Object Storage Integration Design

## Intent & Why

Pengguna ingin mengelola Object Storage (AWS S3, Cloudflare R2, MinIO, dan S3-compatible lainnya) langsung dari antarmuka RusTerm dengan kredensial yang disimpan secara terenkripsi melalui UI, **tanpa perlu menginstal binary `rclone`** di PC lokal maupun di server SSH remote.

Fitur ini ditujukan untuk tiga alur kerja utama dalam satu workspace tab terpisah (`Storage`):
1. **Lokal ↔ Cloud**: manajemen akun/kredensial S3, melihat/membuat/menghapus bucket, menelusuri folder/prefix, serta operasi CRUD object (upload, download, rename/copy, hapus, buat folder, presigned URL).
2. **Server SSH ↔ Cloud**: menyalin/sinkronisasi file atau folder antara server SSH yang terdaftar di RusTerm dengan bucket S3/R2/MinIO tanpa perlu memasang `rclone` atau menyimpan kredensial S3 di server remote.
3. **Cloud ↔ Cloud (S3 ↔ S3)**: menyalin/sinkronisasi object atau folder antar bucket maupun lintas akun/provider (misal MinIO ↔ Cloudflare R2, AWS S3 ↔ MinIO).

## Goals

1. **Manajemen Akun & Kredensial Terenkripsi**:
   - CRUD profil storage (preset: AWS S3, Cloudflare R2, MinIO, Custom S3) melalui UI.
   - `secret_access_key` dienkripsi di SQLite menggunakan `CryptoService` + `Vault` KEK (AES-256-GCM) dan tidak pernah dikembalikan dalam bentuk plaintext ke browser (hanya status/masking).
   - Tombol **Test Koneksi** sebelum menyimpan atau saat mengedit akun.
   - Ikut tersertakan dalam fitur encrypted config backup (`POST /api/config/export` & `POST /api/config/import`) secara backward-compatible.
2. **Manajemen Bucket**:
   - List seluruh bucket yang dapat diakses akun.
   - Buat bucket baru (`CreateBucket`) dan hapus bucket kosong (`DeleteBucket`).
   - Mendukung kasus akun dengan izin terbatas pada satu bucket (`default_bucket`) meskipun `ListBuckets` ditolak (403).
3. **CRUD Object & Prefix (Lokal ↔ Cloud)**:
   - Menelusuri object & folder/prefix (`ListObjectsV2` dengan delimiter `/` dan pagination `continuation_token`), serta filter pencarian prefix.
   - Upload file (single PUT untuk file kecil/menengah, multipart upload untuk file besar ≥ 64 MB).
   - Download file secara streaming.
   - Buat folder (0-byte object berakhiran `/`), rename/copy object, hapus single maupun bulk object.
   - Generate Presigned Download URL berbatas waktu (15 menit / 1 jam / 24 jam) untuk dibagikan atau dipakai cepat.
4. **Transfer Server SSH ↔ Cloud (Tanpa `rclone`)**:
   - Upload file/folder dari server SSH ke bucket S3/R2/MinIO dan download dari bucket ke server SSH.
   - **Direct Mode via Presigned URL (default)**: backend Rust membuat presigned PUT/GET URL berumur pendek, lalu `files_agent.py` (Python3 stdlib) di server SSH mentransfer langsung ke/dari endpoint S3 sehingga traffic tidak melewati PC lokal dan secret key tidak pernah dikirim ke server SSH.
   - **Relay Mode (fallback otomatis & opsi manual)**: jika server SSH berada di jaringan privat tanpa akses langsung ke endpoint S3, atau untuk file > 5 GB yang butuh multipart upload, backend Rust melakukan streaming relay lewat channel SSH ↔ S3.
   - Job tracker dengan progress (file & bytes), status (`queued | running | done | failed | cancelled`), mode aktual (`direct | relay | server_copy`), dan tombol cancel.
5. **Transfer S3 ↔ S3 (Lintas Bucket & Lintas Akun/Provider)**:
   - **Akun sama (lintas bucket)**: memakai server-side `CopyObject` (`x-amz-copy-source: /{src_bucket}/{src_key}`) — instan tanpa lewat bandwidth lokal (`effective_mode = "server_copy"`).
   - **Lintas akun / provider**: backend Rust melakukan *streaming* dari `get_object_stream` (sumber) ke `put_object` / `put_object_multipart` (tujuan) tanpa menyimpan file sementara di disk (`effective_mode = "relay"`).
6. **UI Workspace Terpisah**:
   - Mode workspace baru **`storage`** di `TabBar.tsx` sejajar dengan `terminal | files | containers | database`.

## Non-Goals & Anti-Goals

- **Non-Goals**:
  - Provider non-S3 (Google Drive, OneDrive, Dropbox) pada versi ini.
  - Instalasi, bundling, atau eksekusi binary `rclone`.
  - Pengelolaan IAM policy, CORS, lifecycle rules, atau versioning bucket tingkat lanjut.
  - Penjadwalan backup otomatis berbasis cron (dapat menjadi fase lanjutan).
- **Anti-Goals**:
  - Menyimpan `secret_access_key` plaintext di SQLite, file config di disk, log, atau response API.
  - Mengirim `secret_access_key` ke server SSH remote.
  - Menambahkan dependensi C/OpenSSL yang mempersulit cross-compile Windows (`build-windows.sh`).
  - Membuka endpoint tanpa verifikasi session PIN (`check_auth`).

## Constraints

- **Backend**: Rust, Axum 0.7, Tokio, `reqwest` (`rustls-tls`), `hmac` + `sha2` + `hex` (sudah tersedia di `Cargo.toml`), `quick-xml` (pure-Rust XML parser baru), `rusqlite` (bundled), `russh`.
- **Frontend**: React 18, TypeScript, Vite, CSS kustom RusTerm (tanpa library UI baru).
- **Cross-platform**: Harus berjalan dan terkompilasi bersih untuk Linux dan Windows (`rusterm.exe`).
- **Keamanan**:
  - Semua endpoint `/api/storage/*` wajib melewati `check_auth(&headers, &state.vault)`.
  - Validasi URL endpoint hanya mengizinkan scheme `http://` atau `https://` (IP lokal/LAN/Tailscale diizinkan karena MinIO self-hosted umum berjalan di jaringan privat).
  - Path di server SSH selalu dinormalisasi dan dilindungi oleh `norm()` + `guard()` di `files_agent.py`.
  - Error dari S3/HTTP tidak boleh membocorkan `Authorization` header, signature, atau `secret_access_key`.

## Approaches

### Pendekatan 1: Pure-Rust S3 Client (`reqwest` + SigV4 + `quick-xml`) + Transfer Dual-Mode *(Dipilih)*
- Membangun modul S3-compatible ringan (`services/s3.rs`) di atas `reqwest`, `hmac`, `sha2`, dan `hex` yang sudah ada di `Cargo.toml`, ditambah `quick-xml` untuk parsing XML S3.
- Transfer Server SSH ↔ Cloud memakai **Direct via Presigned URL** melalui `files_agent.py` (Python3 stdlib) dengan fallback **Relay via Rust**.
- **Kelebihan**: Binary tetap ringan dan cepat dikompilasi (termasuk cross-compile Windows); kontrol penuh atas quirk Cloudflare R2 (`region: auto`) dan MinIO (`path-style`, HTTP/HTTPS custom endpoint); transfer VPS ↔ Cloud kencang tanpa lewat bandwidth PC lokal dan tanpa `rclone`.
- **Tradeoff**: Lapisan SigV4, XML parsing, dan multipart upload ditulis dan diuji sendiri di Rust.

### Pendekatan 2: Crate SDK S3 Eksternal (`aws-sdk-s3` / `rust-s3`) + Relay via Rust *(Ditolak)*
- Memakai SDK S3 jadi dan me-relay seluruh transfer SSH ↔ S3 lewat backend Rust.
- **Kelemahan**: `aws-sdk-s3` menambah puluhan crate sehingga compile time & ukuran binary Windows membengkak; crate yang lebih kecil seperti `object_store` kurang mendukung operasi manajemen bucket (`ListBuckets`/`CreateBucket`/`DeleteBucket`); mode relay-only membuat transfer VPS ↔ Cloud terbatas bandwidth lokal.

## Arsitektur & Desain Teknis

### 1. Skema Database SQLite (`backend/src/models/db.rs`)

Menambahkan tabel baru `storage_accounts`:

```sql
CREATE TABLE IF NOT EXISTS storage_accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    provider TEXT NOT NULL DEFAULT 's3',
    endpoint TEXT NOT NULL,
    region TEXT NOT NULL DEFAULT 'us-east-1',
    access_key_id TEXT NOT NULL,
    secret_access_key TEXT NOT NULL,
    default_bucket TEXT NOT NULL DEFAULT '',
    path_style INTEGER NOT NULL DEFAULT 1,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

- `secret_access_key` dienkripsi memakai `CryptoService::encrypt` dengan KEK dari `Vault` sebelum disimpan, dan didekripsi hanya di memori backend saat menandatangani request S3.
- `StorageAccountOutput` mengembalikan `access_key_masked` (atau `access_key_id` publik — `access_key_id` boleh ditampilkan atau dimask sebagian) dan `has_secret: bool` tanpa pernah mengembalikan `secret_access_key`.
- `BackupPayload` di `models/server.rs` & `models/db.rs` ditambahkan field `#[serde(default)] pub storage_accounts: Vec<BackupStorageAccount>` agar export/import tetap backward-compatible dengan backup lama.

### 2. Backend Services

#### a. `backend/src/services/s3.rs`
- **AWS Signature Version 4 (SigV4)**:
  - Header-based signing (`Authorization`, `x-amz-date`, `x-amz-content-sha256`) untuk operasi REST normal.
  - Query-string signing (`X-Amz-Algorithm`, `X-Amz-Credential`, `X-Amz-Date`, `X-Amz-Expires`, `X-Amz-SignedHeaders`, `X-Amz-Signature`) untuk Presigned GET & PUT URL.
- **URL Builder**:
  - `path_style = true` (default): `{endpoint}/{bucket}/{key}` (aman untuk MinIO, Cloudflare R2, dan custom S3).
  - `path_style = false`: `{scheme}://{bucket}.{host}/{key}` (AWS S3 virtual-hosted style).
- **Operasi**:
  - `list_buckets()` → `GET /` (parse `<Bucket><Name>`, `<CreationDate>`).
  - `create_bucket(bucket)` → `PUT /{bucket}` (sertakan `<CreateBucketConfiguration>` jika `region != "us-east-1"` dan `region != "auto"`).
  - `delete_bucket(bucket)` → `DELETE /{bucket}`.
  - `list_objects_v2(bucket, prefix, continuation_token, max_keys)` → `GET /{bucket}?list-type=2&delimiter=/&...` (parse `<CommonPrefixes>` sebagai folder dan `<Contents>` sebagai file).
  - `head_object(bucket, key)` → `HEAD /{bucket}/{key}`.
  - `put_object(bucket, key, body, content_type)` → `PUT /{bucket}/{key}` untuk file < 64 MB.
  - `put_object_multipart(bucket, key, stream, content_type)` → `POST ?uploads` → `PUT ?partNumber=N&uploadId=...` (chunk 16 MB) → `POST ?uploadId=...` (atau `DELETE ?uploadId=...` jika batal/gagal).
  - `get_object_stream(bucket, key)` → `GET /{bucket}/{key}` (streaming `reqwest::Response`).
  - `copy_object(bucket, source_key, dest_key)` → `PUT /{bucket}/{dest_key}` dengan header `x-amz-copy-source`.
  - `delete_object(bucket, key)` → `DELETE /{bucket}/{key}`. Untuk bulk delete, jalankan `delete_object` secara paralel terbatas (maksimal 8 concurrent) agar 100% kompatibel dengan semua versi S3/R2/MinIO tanpa perlu header `Content-MD5`.
  - `presign_url(method, bucket, key, expires_secs)` → mengembalikan URL siap pakai.

#### b. `backend/src/services/storage_transfer.rs` & `backend/src/files_agent.py`
- Mengelola job transfer **Server SSH ↔ S3** di memori (`StorageTransferManager`, pola seragam dengan `services/transfer.rs`).
- **Aksi tambahan di `files_agent.py`** (Python3 stdlib only, tanpa dependensi eksternal):
  - `s3_walk`: mendaftar file reguler di bawah `path` (jika file tunggal, kembalikan 1 item; jika folder, walk rekursif dan kembalikan path relatif + ukuran).
  - `http_put_file`: upload file lokal di server SSH ke presigned PUT URL via `http.client`/`urllib` secara streaming per chunk (1 MB) dengan progress JSON lines ke stdout.
  - `http_get_file`: download dari presigned GET URL langsung ke path tujuan di server SSH (tulis ke file temp lalu atomic rename) dengan progress JSON lines ke stdout.
  - `read_file_raw` / `write_file_raw`: dipakai jika masuk **Relay Mode** (streaming biner lewat channel SSH).
- **Alur Transfer**:
  - **`ssh_to_s3`**:
    1. Panggil `s3_walk` di server SSH untuk menghitung total file & bytes.
    2. Untuk tiap file: jika mode `auto`/`direct` dan ukuran ≤ 5 GB, buat presigned PUT URL (expiry 30 menit) lalu panggil `http_put_file` di server SSH.
    3. Jika `mode == "auto"` dan file pertama gagal dengan network/DNS error di server SSH, otomatis fallback ke **Relay Mode** untuk sisa job dan tandai `effective_mode = "relay"`.
  - **`s3_to_ssh`**:
    1. Panggil `list_objects_v2` rekursif di bawah `s3_prefix` (atau `head_object` jika key tunggal) untuk menghitung total file & bytes.
    2. Untuk tiap object: jika mode `auto`/`direct`, buat presigned GET URL lalu panggil `http_get_file` di server SSH; fallback ke **Relay Mode** jika `auto` dan gagal koneksi jaringan dari server SSH.
  - **`s3_to_s3`**:
    1. Panggil `list_objects_v2` rekursif di bawah `source_prefix` (atau `head_object` jika key tunggal) pada akun sumber untuk menghitung total file & bytes.
    2. Jika `source_account_id == dest_account_id` dan file ≤ 5 GB: panggil `copy_object_across_buckets(src_bucket, src_key, dst_bucket, dst_key)` (`effective_mode = "server_copy"`).
    3. Jika beda akun/provider (atau file > 5 GB): stream `get_object_stream` dari sumber ke `put_object` / `put_object_multipart` di tujuan (`effective_mode = "relay"`).

### 3. Kontrak API Backend (`backend/src/controllers/storage.rs`)

Semua endpoint di-nest di `/api` (dilindungi `check_auth` & `request_guard`):

- **Akun**:
  - `GET /api/storage/accounts` — daftar akun tersimpan (tanpa secret).
  - `POST /api/storage/accounts` — tambah akun baru (enkripsi secret).
  - `PUT /api/storage/accounts/:id` — update akun (secret `None`/kosong mempertahankan secret lama).
  - `DELETE /api/storage/accounts/:id` — hapus akun.
  - `POST /api/storage/accounts/verify` — uji koneksi (`list_buckets`, atau `list_objects_v2(default_bucket, max_keys=1)` jika `default_bucket` diisi dan `list_buckets` mengembalikan 403).
- **Bucket**:
  - `GET /api/storage/accounts/:id/buckets`
  - `POST /api/storage/accounts/:id/buckets` (`{ name }`)
  - `DELETE /api/storage/accounts/:id/buckets/:bucket`
- **Object**:
  - `GET /api/storage/accounts/:id/buckets/:bucket/objects?prefix=&continuation_token=&max_keys=`
  - `POST /api/storage/accounts/:id/buckets/:bucket/folder` (`{ prefix }`)
  - `POST /api/storage/accounts/:id/buckets/:bucket/upload` (multipart: field `prefix` + satu atau lebih `file`)
  - `GET /api/storage/accounts/:id/buckets/:bucket/download?key=` (streaming dengan `Content-Disposition`)
  - `POST /api/storage/accounts/:id/buckets/:bucket/copy` (`{ source_key, dest_key, delete_source }`)
  - `POST /api/storage/accounts/:id/buckets/:bucket/delete` (`{ keys }`)
  - `POST /api/storage/accounts/:id/buckets/:bucket/presign` (`{ key, expires_secs }`)
- **Transfer SSH ↔ S3 & S3 ↔ S3**:
  - `POST /api/storage/transfers` (`direction: "ssh_to_s3" | "s3_to_ssh" | "s3_to_s3"`, dengan field sumber/tujuan SSH atau S3, `mode: "auto" | "direct" | "relay"`, `overwrite`)
  - `GET /api/storage/transfers`
  - `GET /api/storage/transfers/:job_id`
  - `POST /api/storage/transfers/:job_id/cancel`

### 4. Frontend UI (`frontend/src/components/storage/*`)

- **`TabBar.tsx` & `App.tsx`**:
  - Perluas `workspaceMode` menjadi `"terminal" | "files" | "containers" | "database" | "storage"`.
  - Render `<StorageWorkspace servers={servers} />` saat `workspaceMode === "storage"`.
- **Komponen baru**:
  - `StorageWorkspace.tsx`: layout 2-kolom (panel kiri: daftar akun S3/R2/MinIO + daftar bucket akun terpilih dengan tombol tambah/hapus; panel kanan: breadcrumb prefix, toolbar Upload / Buat Folder / Sync SSH / Refresh / Filter, tabel object dengan aksi Download, Rename/Copy, Presigned URL, Delete & Bulk Delete, serta bar/drawer Transfer Jobs yang polling saat ada job aktif).
  - `StorageAccountModal.tsx`: modal tambah/edit akun dengan preset provider (AWS S3, Cloudflare R2, MinIO, Custom S3), auto-fill default region/path-style per preset, tombol **Test Koneksi**, dan validasi form.
  - `StorageBucketModal.tsx`: modal buat bucket baru dengan validasi format nama bucket.
  - `StorageTransferModal.tsx`: modal buat transfer SSH ↔ S3 (pilih arah, server SSH, path SSH, bucket & prefix S3, mode `Auto` / `Relay`).
- **API & Types**:
  - `frontend/src/storageApi.ts` — fungsi fetch terketik untuk semua endpoint `/api/storage/*`.
  - `frontend/src/types.ts` — tipe `StorageProvider`, `StorageAccount`, `StorageAccountInput`, `StorageBucket`, `StorageObject`, `StorageListOut`, `StorageTransferJob`, dll.

### 5. Keputusan Implementasi (Diserahkan ke Builder / Ditentukan Default)
- **Bulk delete**: memakai paralel `DELETE` terbatas (concurrency 8) alih-alih `POST /?delete` agar tidak perlu crate `md5` tambahan dan 100% kompatibel dengan MinIO/R2.
- **Batas single PUT vs Multipart upload**: threshold 64 MB (di bawah 64 MB pakai single PUT, ≥ 64 MB pakai multipart 16 MB/part).
- **Polling Transfer Jobs**: interval 1 detik saat ada job berstatus `queued`/`running`, berhenti otomatis saat semua job selesai.
- **Fallback `default_bucket`**: jika `ListBuckets` ditolak (403) namun `default_bucket` terisi, UI tetap menampilkan `default_bucket` agar token scoped per-bucket (sangat umum di Cloudflare R2 & AWS IAM) tetap bisa digunakan.
