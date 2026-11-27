# S3 / R2 / MinIO Object Storage Integration Implementation Plan

> **For agentic workers:** Executed inline in this session via `executing-plans` + `test-driven-development` (no subagent delegation per user rules). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a pure-Rust S3/R2/MinIO object storage manager in RusTerm (encrypted vault credentials, bucket/object CRUD, SSH ↔ S3 and S3 ↔ S3 transfers without `rclone`) exposed as a dedicated `storage` workspace tab.

**Architecture:** Backend adds `storage_accounts` (AES-256-GCM encrypted `secret_access_key` via `Vault`), a lightweight SigV4 + `reqwest` + `quick-xml` S3 client (`services/s3.rs`), a transfer job manager (`services/storage_transfer.rs`) using `files_agent.py` presigned-URL direct transfer with Rust relay fallback and S3-to-S3 server-copy/streaming, and Axum routes under `/api/storage/*`. Frontend adds a new `workspaceMode: "storage"` in `TabBar.tsx`/`App.tsx` backed by `StorageWorkspace.tsx` and dedicated account/bucket/transfer modals.

**Tech Stack:** Rust (Axum 0.7, Tokio, Reqwest `rustls-tls`, `hmac`, `sha2`, `hex`, `quick-xml`, Rusqlite bundled, Russh), Python3 stdlib (`files_agent.py`), React 18, TypeScript, Vite, Vitest.

**Spec:** `docs/superpowers/specs/2026-11-27-s3-storage-integration-design.md`

## Global Constraints

- Zero `rclone` dependency on local machine or remote SSH servers.
- Pure-Rust dependencies only (no C/OpenSSL) so Linux and Windows (`build-windows.sh`) builds stay intact.
- `secret_access_key` encrypted at rest in SQLite via `CryptoService` + `Vault` KEK; never returned plaintext in API responses or logs; never sent to SSH servers.
- All `/api/storage/*` endpoints require `check_auth(&headers, &state.vault)`.
- Endpoint URL scheme must be strictly `http://` or `https://`.
- Config backup (`config/export` & `config/import`) must remain backward-compatible with backups lacking `storage_accounts`.

## Review Focus

1. **Special characters & spaces in S3 keys**: SigV4 canonical URI and query encoding must RFC-3986 percent-encode spaces, `+`, unicode, and slashes properly without double-encoding `/`.
2. **Scoped R2/IAM tokens lacking `ListBuckets` permission (403)**: `verify` and `list_buckets` must gracefully fall back to `default_bucket` when configured.
3. **Secret leakage on update or error**: Updating an account with `secret_access_key: None` or `""` must preserve the existing encrypted secret; S3 HTTP errors must strip auth headers/signatures.
4. **S3 ↔ S3 same-account vs cross-account & self-overwrite**: Same-account copy uses server-side `CopyObject` (`effective_mode = "server_copy"`); cross-account streams via Rust (`effective_mode = "relay"`); copying a key onto itself on the same bucket must be rejected cleanly.
5. **Path traversal on SSH destination (`s3_to_ssh`)**: S3 keys containing `..` or leading `/` must be sanitized before joining with `ssh_path`, and `files_agent.py` `norm()`/`guard()` must prevent writing to protected system paths.

---

### Task 1: Storage Models, SQLite `storage_accounts` & Encrypted Backup

**Files:**
- Create: `backend/src/models/storage.rs`
- Modify: `backend/src/models/mod.rs`
- Modify: `backend/src/models/server.rs` (`BackupStorageAccount`, `BackupPayload`, `BackupStats`)
- Modify: `backend/src/models/db.rs` (`storage_accounts` table, CRUD, decrypt, backup export/import)
- Test: `backend/tests/storage_db_test.rs`

**Interfaces:**
- Produces: `StorageAccountOutput`, `StorageAccountInput`, `StorageAccountUpdateInput`, `StorageAccountVerifyInput`, `DecryptedStorageAccount`, `DbStore::{list_storage_accounts, create_storage_account, update_storage_account, delete_storage_account, get_decrypted_storage_account}`.

- [ ] **Step 1:** Write failing integration tests in `backend/tests/storage_db_test.rs` covering create/list (secret masked/absent), decrypt round-trip, update preserving secret when `None`/`""`, delete, and encrypted backup export/import (including backward compatibility when `storage_accounts` is missing).
- [ ] **Step 2:** Run `cargo test --test storage_db_test` (expect compile failure).
- [ ] **Step 3:** Implement `backend/src/models/storage.rs`, update `models/mod.rs`, `models/server.rs`, and `models/db.rs`.
- [ ] **Step 4:** Run `cargo test --test storage_db_test` (expect PASS).

---

### Task 2: Pure-Rust S3 Client (SigV4, Presigning, XML Parsing, Bucket & Object CRUD)

**Files:**
- Modify: `backend/Cargo.toml` (add `quick-xml = "0.36"` and `url = "2"`)
- Create: `backend/src/services/s3.rs`
- Modify: `backend/src/services/mod.rs`
- Test: `backend/tests/s3_client_test.rs`

**Interfaces:**
- Consumes: `DecryptedStorageAccount`, `StorageBucket`, `StorageObject`, `StorageListOut`.
- Produces: `S3Client`, `S3Error`, `validate_endpoint_url`, `validate_bucket_name`, `S3Client::{list_buckets, create_bucket, delete_bucket, list_objects_v2, head_object, put_object, put_object_multipart, get_object_stream, copy_object_across_buckets, delete_object, delete_objects_parallel, presign_url, verify_account}`.

- [ ] **Step 1:** Add `quick-xml` and `url` to `backend/Cargo.toml` and write failing tests in `backend/tests/s3_client_test.rs` covering:
  - SigV4 canonical request & signature generation (including spaces & unicode in keys).
  - Presigned GET & PUT URL generation (query params & path-style vs virtual-host).
  - Endpoint scheme validation (rejects `file://`, `ftp://`, accepts `http://127.0.0.1:9000` and `https://...`).
  - Bucket name validation.
  - Mock local Axum S3 HTTP server testing `list_buckets`, `create_bucket`, `delete_bucket`, `list_objects_v2` (folders + objects + pagination), `put_object`, `get_object_stream`, `copy_object_across_buckets`, `delete_objects_parallel`, and `verify_account` fallback to `default_bucket` on 403.
- [ ] **Step 2:** Run `cargo test --test s3_client_test` (expect compile failure).
- [ ] **Step 3:** Implement `backend/src/services/s3.rs` and export in `backend/src/services/mod.rs`.
- [ ] **Step 4:** Run `cargo test --test s3_client_test` (expect PASS).

---

### Task 3: Transfer Engine (`ssh_to_s3`, `s3_to_ssh`, `s3_to_s3`) & `files_agent.py` Actions

**Files:**
- Modify: `backend/src/files_agent.py` (add `s3_walk`, `http_put_file`, `http_get_file` using Python3 stdlib only)
- Create: `backend/src/services/storage_transfer.rs`
- Modify: `backend/src/services/mod.rs`
- Test: `backend/tests/storage_transfer_test.rs`

**Interfaces:**
- Consumes: `S3Client`, `SshPool`, `DecryptedServerCreds`, `DecryptedStorageAccount`.
- Produces: `StorageTransferManager`, `StorageTransferJob`, `StorageTransferRequest`, `sanitize_relative_key`.

- [ ] **Step 1:** Write failing tests in `backend/tests/storage_transfer_test.rs` covering:
  - `sanitize_relative_key` rejecting `..` / absolute traversal segments.
  - `s3_to_s3` same-account server-side copy (`effective_mode == "server_copy"`) and rejecting self-copy (`src_bucket == dst_bucket && src_key == dst_key`).
  - `s3_to_s3` cross-account streaming (`effective_mode == "relay"`) against two mock S3 endpoints.
  - `files_agent.py` Python unit check via `python3` for `s3_walk`, `http_put_file`, and `http_get_file` against a local mock HTTP server.
- [ ] **Step 2:** Run `cargo test --test storage_transfer_test` (expect fail).
- [ ] **Step 3:** Implement `files_agent.py` actions and `backend/src/services/storage_transfer.rs`.
- [ ] **Step 4:** Run `cargo test --test storage_transfer_test` (expect PASS).

---

### Task 4: Storage HTTP Controller `/api/storage/*` & `AppState` Wiring

**Files:**
- Create: `backend/src/controllers/storage.rs`
- Modify: `backend/src/controllers/mod.rs`
- Modify: `backend/src/controllers/servers.rs` (`AppState` adds `s3: S3Client`, `storage_transfers: StorageTransferManager`)
- Modify: `backend/src/main.rs` (initialize & nest `storage::router`)
- Test: `backend/tests/storage_api_test.rs`

**Interfaces:**
- Produces: Axum router for `/api/storage/accounts`, `/api/storage/accounts/verify`, `/api/storage/accounts/:id/buckets`, `/api/storage/accounts/:id/buckets/:bucket/{objects,folder,upload,download,copy,delete,presign}`, `/api/storage/transfers`.

- [ ] **Step 1:** Write failing API integration tests in `backend/tests/storage_api_test.rs` verifying auth rejection (401 without session), account CRUD + verify, bucket list/create/delete, object upload/list/download/copy/delete/presign, and `s3_to_s3` transfer job lifecycle against a mock S3 server.
- [ ] **Step 2:** Run `cargo test --test storage_api_test` (expect compile failure).
- [ ] **Step 3:** Implement `backend/src/controllers/storage.rs`, update `controllers/mod.rs`, `servers.rs`, `main.rs`, and any existing test helpers constructing `AppState`.
- [ ] **Step 4:** Run `cargo test --test storage_api_test` and full `cargo test` (expect PASS).

---

### Task 5: Frontend Types, Storage API Client & Workspace UI (`storage`)

**Files:**
- Modify: `frontend/src/types.ts` (add Storage types & `BackupStats` fields)
- Create: `frontend/src/storageApi.ts`
- Create: `frontend/src/storageHelpers.ts` (provider presets, bucket name validation, breadcrumb/key formatting)
- Create: `frontend/src/components/storage/StorageAccountModal.tsx`
- Create: `frontend/src/components/storage/StorageBucketModal.tsx`
- Create: `frontend/src/components/storage/StorageTransferModal.tsx`
- Create: `frontend/src/components/storage/StorageWorkspace.tsx`
- Modify: `frontend/src/components/TabBar.tsx` (add `"storage"` workspace mode button)
- Modify: `frontend/src/App.tsx` (wire `workspaceMode: "storage"` and `<StorageWorkspace />`)
- Modify: `frontend/src/components/Icon.tsx` (add `storage` / bucket icon if needed)
- Modify: `frontend/src/styles.css` (styles for StorageWorkspace & modals)
- Test: `frontend/tests/storage.test.ts`

- [ ] **Step 1:** Write failing frontend tests in `frontend/tests/storage.test.ts` testing provider presets (AWS S3, Cloudflare R2 `auto`, MinIO `path_style`), bucket name validation, breadcrumb/prefix parsing, and `storageApi` request formatting.
- [ ] **Step 2:** Run `cd frontend && npm test -- --run tests/storage.test.ts` (expect FAIL).
- [ ] **Step 3:** Implement `types.ts`, `storageHelpers.ts`, `storageApi.ts`, all `components/storage/*` components, `TabBar.tsx`, `App.tsx`, `Icon.tsx`, and `styles.css`.
- [ ] **Step 4:** Run `cd frontend && npx tsc --noEmit && npm test -- --run && npm run build` (expect PASS).

---

### Task 6: Final Verification & Security Audit

- [ ] Run full backend suite: `cd backend && cargo test`
- [ ] Run full frontend checks: `cd frontend && npx tsc --noEmit && npm test -- --run && npm run build`
- [ ] Perform inline security audit (IDOR/Auth, Injection, Secrets & Logs, Dependencies) and check `git diff --stat`.
