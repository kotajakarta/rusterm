# Design: Database Client Fase 3 (Quick-Connect Container, Remote SQLite via SSH, Config Backup)

## 1. Tujuan
Melengkapi Database Client RusTerm dengan tiga integrasi ekosistem:
1. **Quick-Connect dari Container** — tombol `DB` di panel `Containers` untuk container PostgreSQL / MySQL / MariaDB yang sedang berjalan, membaca metadata & environment variable via `docker/podman inspect` untuk pre-fill form koneksi DB.
2. **Remote SQLite via SSH** — membuka dan mengedit file `.sqlite`/`.db` langsung di server remote lewat `python3` (`sqlite3` stdlib) via `SshPool::exec_capture` tanpa download/upload file.
3. **Config Backup (`db_connections`)** — menyertakan koneksi DB terenkripsi ke dalam backup `.tdk` dan Cloud Sync (Cloudflare KV) secara backward-compatible.

## 2. Pendekatan yang Dipilih
- **Container Quick-Connect**: Endpoint `GET /api/containers/db-hint?server_id=&container_id=` menjalankan `<engine> inspect` di remote, mem-parsing image, port mapping, IP internal, dan env standar (`POSTGRES_*`, `MYSQL_*`, `MARIADB_*`) tanpa log kredensial.
- **Remote SQLite**: `RemoteSqliteDriver` mengimplementasikan `DbDriver` dan mengirim request JSON lewat stdin ke script `python3` inline (mencegah command injection, mendukung WAL mode live, dan menegakkan `mode=ro` saat `read_only = true`).
- **Config Backup**: Tambah `#[serde(default)] pub db_connections: Vec<BackupDbConnection>` di `BackupPayload` dan `db_connections_added`/`db_connections_updated` di `BackupStats`. Relasi `ssh_server_id` dipetakan lewat `(ssh_server_name, ssh_server_host)`.

## 3. Verifikasi
- Backend `cargo test`:
  - Test backup/restore `db_connections` termasuk re-linking `ssh_server_id` dan kompatibilitas payload lama.
  - Test `parse_container_db_hint` untuk PostgreSQL, MySQL, MariaDB, port unpublished, dan non-DB container.
  - Test agent Python Remote SQLite (full CRUD, pagination/filter, DDL, dan guard `read_only`).
- Frontend `npm run typecheck` & `npm test`:
  - Unit test deteksi DB container (`detectDbEngineFromContainer`) dan `formatSyncStats` untuk statistik `db_connections`.
