# Transfer, Host Key & LAN Hardening Implementation Plan

> **For agentic workers:** Execute inline with superpowers:executing-plans (user rule: no subagents). Steps use checkbox (`- [ ]`) syntax for tracking. No commits unless the user asks.

**Goal:** Make Remote Sync / Direct Transfer actually move files, verify SSH host keys (trust on first use), and close the app to the LAN by default.

**Architecture:** Transfers run as background jobs in a `TransferManager` held in `AppState`; each job pumps the files agent's `pack` tar stream from the source server into `unpack` on the destination, with a credit protocol so memory stays bounded. Host keys are checked inside the russh client handler against `servers.host_key`, stored on first connect. A request guard middleware enforces the per-process token and loopback `Host`, the server binds `127.0.0.1` unless `--host` says otherwise, and the vault rate-limits PIN attempts.

**Tech Stack:** Rust (axum 0.7, tokio, russh 0.45, rusqlite), Python 3 stdlib files agent, React frontend (unchanged contract).

**Spec:** No separate design doc (user opted out of the Q&A, 2026-11-11). Decisions are recorded here and in the chat: frontend contracts in `frontend/src/types.ts` (`TransferJob`, `TransferScan`, `DirectTransferScanIn`, `DirectTransferStartIn`) are the source of truth.

## Global Constraints

- Frontend API contract must not change; backend adapts to `frontend/src/types.ts` and `frontend/src/api.ts`.
- Error responses use JSON `{"detail": "..."}` (already enforced by `api_errors::wrap_plain_errors`).
- No new crates.
- Every remote path travels inside agent JSON, never concatenated into a shell command.
- Default bind address `127.0.0.1`; `--host <addr>` opts into other interfaces.
- `/api/auth/status` stays reachable without token (single-instance detection, `app_launcher::check_existing_instance`).

## Review Focus

- Source server faster than destination → backend memory must stay bounded (credit window, Task A1/A3).
- User presses Cancel mid-transfer → job becomes `cancelled`, remote agents exit, no half-written temp files left (agent already writes via tmp + rename) (Task A3).
- Path selected outside the transfer root, or sibling prefix like `/var/www2` vs root `/var/www` → 400, never a wrong relative path (Task A2).
- Server reinstalled (host key changed) → clear error naming both fingerprints and the "reset host key" action, not a generic SSH error (Task B2).
- Request with wrong `Host` header (DNS rebinding) or missing token → 403 JSON, while the bundled UI, file download links (`?token=`) and WebSocket keep working (Task C2).

---

## Part A — Remote Sync / Direct Transfer

### Task A1: Credit flow control in agent `pack`

**Files:**
- Modify: `backend/src/files_agent.py` (`op_pack`)
- Test: `backend/tests/transfer_agent_test.rs`

**Interfaces:**
- Produces: pack request header `{"op":"pack","root":str,"rels":[str],"ignore":[str],"read_gitignore":bool,"credit":true}` + `\n`. With `credit` true, after every `CREDIT_BYTES = 1048576` bytes written the agent flushes stdout and reads one byte from stdin; EOF on stdin aborts (`AgentError("Transfer cancelled")`, JSON on stderr, exit 1). Without `credit`, behaviour unchanged.

- [ ] Step 1: Tests (run agent locally via `python3 -c FILES_AGENT_SRC`): `pack_with_credits_streams_complete_tar` (tree with a 3 MiB file + subdir + symlink; stdin = header + 64 credit bytes; stdout parses as tar with all members, file bytes identical); `pack_without_credits_aborts_when_stdin_closes` (header only, then EOF; exit code 1, stderr JSON `ok:false`, error contains "cancelled").
- [ ] Step 2: Run, expect failure (second test hangs-free failure: current pack ignores credit and exits 0).
- [ ] Step 3: Implement a `_CreditWriter` file-like wrapper used by `op_pack` when `req.get("credit")`.
- [ ] Step 4: Tests pass; `python3 -m py_compile`.

### Task A2: Job model, manager and path helpers

**Files:**
- Create: `backend/src/services/transfer.rs`
- Modify: `backend/src/services/mod.rs`
- Test: unit tests in `transfer.rs`

**Interfaces:**
- Produces:
  - `pub struct TransferJob` serializing exactly as TS `TransferJob` (fields `id, remote_id: Option<i64>, remote_name, direction, state, source_server_id, source_root, dest_server_id, dest_root, rels, read_gitignore, sent_bytes, total_bytes, total_files, result: Option<TransferResult>, error: Option<String>`), `pub struct TransferResult { files, dirs, links, bytes: u64 }`.
  - `pub struct TransferSpec { remote_id: Option<i64>, remote_name: String, direction: String, source_server_id, source_root, dest_server_id, dest_root, rels: Vec<String>, ignore: Vec<String>, read_gitignore: bool, move_sources: bool }`.
  - `#[derive(Clone, Default)] pub struct TransferManager` with `create(&self, spec: &TransferSpec) -> (TransferJob, JobHandle)`, `get(&self, id: &str) -> Option<TransferJob>`, `cancel(&self, id: &str) -> Option<TransferJob>`; `JobHandle` exposes `update(|&mut TransferJob|)`, `is_cancelled() -> bool`, `cancelled() -> impl Future` (tokio `Notify`).
  - `pub fn relative_paths(root: &str, paths: &[String]) -> Result<Vec<String>, String>`: normalizes, `root` itself → `"."`, rejects paths outside root with a path-component boundary check (`/var/www2` is outside `/var/www`).
  - `pub const CREDIT_BYTES: u64 = 1_048_576; pub const INITIAL_CREDITS: usize = 8;`
- [ ] Step 1: Tests: `relative_paths` (root→".", child, nested, trailing slash, `/var/www2` rejected, `..` escape rejected); manager create/get/cancel transitions (`running`→`cancelled`, cancel on finished job leaves state); serde JSON keys of `TransferJob` equal the TS field list.
- [ ] Step 2: Fail, implement, pass.

### Task A3: Streaming pump

**Files:**
- Modify: `backend/src/services/transfer.rs` (`pub async fn run_transfer(pool: SshPool, src: DecryptedServerCreds, dst: DecryptedServerCreds, spec: TransferSpec, job: JobHandle)`)
- Modify: `backend/src/services/ssh_pool.rs` (make `open_channel` usable; already `pub`)

**Interfaces:**
- Consumes: `SshPool::open_channel`, `build_agent_command`, `SshPool::run_agent`, Task A1 header, Task A2 types.
- Behaviour: (1) `scan` on source → `total_bytes = tar_bytes`, `total_files = files`; (2) open dst channel exec agent, send unpack header `{"op":"unpack","root":dest_root}\n`; (3) open src channel exec agent, send pack header with `credit:true`, then `INITIAL_CREDITS` credit bytes, no EOF; (4) loop `select!{ cancelled, src.wait() }`: Data → `dst.data()`, `sent_bytes += n`, send one credit byte per `CREDIT_BYTES` forwarded; ExtendedData → collect stderr; Close/None → stop; (5) source exit≠0 → error from stderr JSON; else `dst.eof()`, read dst stdout JSON → `result` or error; (6) `move_sources` and success → agent `delete` of `root/rel` for each rel (not "."); (7) cancel → close both channels, state `cancelled`. Always close both channels.
- [ ] Step 1: Verification is end-to-end (needs sshd): temporary non-root sshd on 127.0.0.1:22022 (scratchpad), upload dir tree incl. 30 MB file, file with spaces, symlink, `.gitignore` excluded entry; assert byte-identical result, `.gitignore`d file absent, `move` deletes source, cancel mid-transfer yields `cancelled` and no `.tukang-tmp-*` left behind.

### Task A4: Endpoints

**Files:**
- Modify: `backend/src/controllers/files.rs` (`scan_remote`, `start_remote_transfer`, `get_transfer_job`, `cancel_transfer_job`, `direct_transfer_scan`, `direct_transfer_start`)
- Modify: `backend/src/controllers/servers.rs` (`AppState` gains `pub transfers: TransferManager`), `backend/src/main.rs`, test constructors in `backend/tests/{paste_image,containers,files,cloudflare,system}_test.rs`
- Test: `backend/tests/files_test.rs`

**Interfaces:**
- `scan_remote` uses `relative_paths` (400 on outside root), sends `read_gitignore: true`, returns scan + `source_server_id, source_root, dest_server_id, dest_root, rels, has_gitignore`.
- `direct_transfer_scan(DirectTransferScanIn)` → same shape, `ignore` default `[]`, `use_gitignore` default `true` echoed back.
- `start_remote_transfer` → `TransferJob` (`remote_name` = remote name, `direction` = upload|download); `direct_transfer_start` → `TransferJob` (`remote_name` = "Direct transfer", `direction` = mode copy|move, `move_sources` = mode=="move"). Both spawn `run_transfer`.
- `get_transfer_job` / `cancel_transfer_job` → `TransferJob`, 404 for unknown id.
- [ ] Step 1: Router tests: 401 without session; unknown job → 404 JSON; direct scan with path outside `source_dir` → 400; start for unknown remote → 404.
- [ ] Step 2: Fail, implement, pass; full E2E from Task A3 through the HTTP endpoints.

## Part B — SSH Host Key Verification (TOFU)

### Task B1: DB host key storage

**Files:** Modify `backend/src/models/db.rs`; Test `backend/tests/api_test.rs`
- Produces: `get_host_key(&self, id: i64) -> Result<Option<String>, DbError>`, `set_host_key(&self, id: i64, key: &str, fingerprint: &str) -> Result<(), DbError>`; `update_server` clears `host_key`/`host_key_fp` when host or port changes.
- [ ] Test `test_host_key_storage_and_reset_on_host_change`: set → get returns key and `list_servers` shows fp; update with same host keeps it; update with new host clears it.

### Task B2: Verify in the SSH handler

**Files:** Modify `backend/src/services/ssh_pool.rs`, `backend/src/main.rs`; Test `backend/tests/ssh_pool_test.rs`
- Produces: `pub fn host_key_string(key: &PublicKey) -> String` (`"<algo> <base64>"`), `pub fn host_key_fingerprint(key: &PublicKey) -> String` (`"SHA256:<b64>"`), `pub fn check_host_key(expected: Option<&str>, presented: &PublicKey) -> HostKeyCheck` with `enum HostKeyCheck { Trusted, FirstUse, Mismatch { expected_fp: String, presented_fp: String } }`; `SshError::HostKeyMismatch(String)`; `SshPool::with_host_keys(db: DbStore) -> SshPool` (main uses it; `SshPool::new()` keeps accept-all for tests). `connect_new` stores the key on `FirstUse` when `creds.id > 0`, after authentication succeeds.
- Mismatch message (Indonesian): `Host key server berubah! Tersimpan {expected_fp}, diterima {presented_fp}. Jika server memang di-install ulang, buka Edit server lalu klik "Reset host key".`
- [ ] Tests with generated ed25519 keys: same key → Trusted; None → FirstUse; other key → Mismatch with both fingerprints; fingerprint starts with `SHA256:`.
- [ ] E2E with temp sshd: first connect stores fp (visible in `GET /api/servers`), swap sshd host key → terminal/WS and stats return the mismatch message; `reset-hostkey` → connects again.

## Part C — LAN Hardening

### Task C1: CLI `--host` / `--token`

**Files:** Create `backend/src/cli.rs` (`pub struct CliArgs { port: u16, host: String, data_dir, dist_dir, no_browser: bool, token: Option<String> }`, `pub fn parse_args(args: &[String]) -> CliArgs`), modify `backend/src/lib.rs`, `backend/src/main.rs`; Test `backend/tests/cli_test.rs`.
- Defaults: port 1945, host `127.0.0.1`. `--token` overrides the random token (dev with Vite `?token=`).
- [ ] Tests: defaults; all flags; bad port falls back to 1945.

### Task C2: Request guard

**Files:** Create `backend/src/controllers/request_guard.rs`; modify `backend/src/main.rs`; Test `backend/tests/request_guard_test.rs`.
- Produces: `#[derive(Clone)] pub struct GuardConfig { pub token: String, pub loopback_only: bool }`, `pub async fn guard(State(cfg): State<GuardConfig>, req: Request, next: Next) -> Response` applied to the whole app.
- Rules: if `loopback_only`, `Host` must be `127.0.0.1`, `localhost` or `[::1]` (any port) → else 403. For `/api/*` except `/api/auth/status`: token from `X-Token` header or `token` query param must equal config token (constant-time compare) → else 403 `{"detail":"Token tidak valid"}`. Static files and `/ws/*` need only the Host rule (WS still requires session).
- [ ] Tests: good token header ok; `?token=` ok; missing/wrong token 403 JSON; `/api/auth/status` without token ok; Host `evil.com` 403 when loopback_only, allowed when not; static `/` ok without token.

### Task C3: PIN rate limit and authenticated lock

**Files:** Modify `backend/src/services/vault.rs`, `backend/src/controllers/auth.rs`; Test `backend/tests/vault_test.rs`.
- Rules: after 5 consecutive wrong PINs, further attempts are refused for 30 s, doubling per further failure, capped at 15 min; success resets. Refusal error `VaultError::TooManyAttempts(u64 secs)` → HTTP 429 `Terlalu banyak percobaan PIN. Coba lagi dalam {n} detik.` Argon2 verification runs via `spawn_blocking` without holding the write lock. `POST /api/auth/lock` requires a valid session (401 otherwise).
- [ ] Tests: 5 wrong → 6th returns TooManyAttempts even with the right PIN; correct PIN after lockout window works (inject clock via `Vault::login_at(pin, now: Instant)` used by `login`); lock without session → 401.

## Final verification

- `cargo test --no-fail-fast` (only pre-existing `static_test::test_embedded_asset_serving_and_token_injection` may fail), `python3 -m py_compile backend/src/files_agent.py`, `npm run build` in `frontend`, E2E scripts above, `./build-windows.sh`, security table, `git diff --stat`.
