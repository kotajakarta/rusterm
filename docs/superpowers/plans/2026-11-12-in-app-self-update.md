# In-App Self-Update with Process Termination Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task inline in the current session. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement one-click native in-app self-update for RusTerm on Windows/Linux with real-time download progress tracking, termination of lingering background processes, in-place binary rename, and automatic application restart.

**Architecture:**
- Backend `backend/src/services/updater.rs`: Manages update states, streaming download from GitHub Releases with chunked progress reporting, killing stale background `rusterm.exe`/`termdeck.exe` processes (excluding current PID) via `taskkill`, performing the Windows atomic file rename trick (`.exe` -> `.exe.old`, `.exe.new` -> `.exe`), launching the updated executable, and terminating the running process.
- Backend `backend/src/controllers/system.rs`: Exposes `/api/system/update-apply` and `/api/system/update-progress` endpoints guarded by vault session token.
- Backend `backend/src/main.rs`: Adds silent startup cleanup for any stale `*.exe.old` files.
- Frontend `frontend/src/components/UpdateModal.tsx`: Adds "Perbarui Sekarang" action button, animated progress bar, transfer rate/bytes counter, state transitions (`downloading` -> `applying` -> `completed`), and automatic reload.

**Tech Stack:** Rust (Axum, Tokio, Reqwest, Std Process), TypeScript, React, Vite.

---

## Global Constraints
- Standalone portable executable: no external updater GUI/CLI packages required.
- File lock safety: must handle Windows file locking by renaming the active executable and terminating other background instances before replacing.
- Zero plaintext secrets: token/session validation on all API calls.
- Pure inline execution without subagents.

---

## Review Focus
1. **Windows File Lock**: Active `rusterm.exe` cannot be overwritten in-place, must be renamed to `rusterm.exe.old`.
2. **Stale Background Instances**: Lingering `rusterm.exe` processes blocking the file or port must be killed with `taskkill /F /IM rusterm.exe /FI "PID ne <pid>"`.
3. **Corrupted/Incomplete Downloads**: Must validate binary integrity (non-zero size, valid executable magic header) before replacing current binary.
4. **Network/Rate Limit Failures**: Graceful error reporting to UI with retry button and fallback to manual download.
5. **Clean Startup**: Leftover `.old` files from previous updates must be removed silently on startup.

---

## Tasks

### Task 1: Backend Self-Update Service & State Management
**Files:**
- Create: `backend/src/services/updater.rs`
- Modify: `backend/src/services/mod.rs`
- Test: `backend/tests/updater_test.rs`

- [ ] Step 1.1: Write failing unit test `backend/tests/updater_test.rs` testing updater progress state transitions and binary validation.
- [ ] Step 1.2: Run `cargo test --test updater_test --manifest-path backend/Cargo.toml` and confirm failure.
- [ ] Step 1.3: Implement `backend/src/services/updater.rs` with `UpdateProgressResult`, `kill_other_instances()`, streaming downloader with `tokio::sync::RwLock`, binary validation, file rename logic, and spawn+exit.
- [ ] Step 1.4: Register module in `backend/src/services/mod.rs`.
- [ ] Step 1.5: Run `cargo test --test updater_test --manifest-path backend/Cargo.toml` and confirm it passes.

### Task 2: Backend System Controller & Startup Cleanup
**Files:**
- Modify: `backend/src/controllers/system.rs`
- Modify: `backend/src/main.rs`
- Test: `backend/tests/system_test.rs`

- [ ] Step 2.1: Connect `/system/update-apply` and `/system/update-progress` to `updater` service in `backend/src/controllers/system.rs`.
- [ ] Step 2.2: Add startup cleanup function `cleanup_old_executables()` in `backend/src/main.rs`.
- [ ] Step 2.3: Update and run `backend/tests/system_test.rs` to verify authorization and response payloads.
- [ ] Step 2.4: Run all backend tests `cargo test --manifest-path backend/Cargo.toml` to ensure 0 regressions.

### Task 3: Frontend Update Modal UI & Progress Polling
**Files:**
- Modify: `frontend/src/components/UpdateModal.tsx`
- Modify: `frontend/src/updateLogic.ts`
- Test: `frontend/tests/update.test.ts`

- [ ] Step 3.1: Add unit tests in `frontend/tests/update.test.ts` for update progress calculation and status formatting.
- [ ] Step 3.2: Run `npx tsx --test tests/update.test.ts` to confirm test behavior.
- [ ] Step 3.3: Update `frontend/src/components/UpdateModal.tsx` to include "Perbarui Sekarang" button, progress bar showing downloaded/total bytes and percentage, state transitions, and error handling.
- [ ] Step 3.4: Verify frontend TypeScript build with `npm run build`.

### Task 4: Full System Verification, Release Packaging & Deployment
**Files:**
- Output: `dist-release/rusterm.exe`
- Output: `dist-release/rusterm-v0.2.4-windows-x64.zip`

- [ ] Step 4.1: Run full test suites (cargo test + npm test).
- [ ] Step 4.2: Build Windows standalone package using `./build-windows.sh`.
- [ ] Step 4.3: Commit changes with descriptive commit message.
- [ ] Step 4.4: Push to GitHub and update release assets.
