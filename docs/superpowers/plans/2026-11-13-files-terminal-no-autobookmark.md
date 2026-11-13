# Files Terminal Tanpa Auto-Bookmark Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Tombol Terminal di file manager membuka terminal di folder tanpa membuat path sidebar; bookmark file manager = path sidebar (satu sumber data).

**Architecture:** `/ws/terminal` menerima `path` opsional untuk `cd` awal (via `StartupGate`). Frontend menurunkan bookmark file manager dari state `bookmarks` App; toggle memanggil API lalu `reload()`.

**Tech Stack:** Rust (axum), React + TypeScript, `node --test`.

**Spec:** `docs/superpowers/specs/2026-11-13-files-terminal-no-autobookmark-design.md`

## Global Constraints
- Path dari query hanya menghasilkan `build_cd_command(path, "")` — tidak pernah startup command bebas.
- Bookmark milik server lain tidak dipakai (perilaku lama dipertahankan).
- Preset Home tetap memakai `openOrCreatePresetPath`.

## Review Focus
- Path dengan spasi/kutip/`$()` → di-quote (sudah dijamin `build_cd_command`, dites di Task 1).
- `path` kosong/whitespace → tidak ada `cd` (Task 1).
- Path dengan trailing slash vs bookmark tersimpan tanpa slash → tetap dikenali sebagai bookmark (Task 2, `normalizeBookmarkPath`).
- Reconnect tab (session bump) dengan `startPath` → `cd` lagi ke folder yang sama (URL memuat `path` dari tab).
- Bookmark dihapus di sidebar saat file manager terbuka → ikon bookmark hilang tanpa reload (state turunan dari App).

---

### Task 1: Backend — `path` query di terminal WS

**Files:**
- Modify: `backend/src/controllers/terminal_ws.rs` (WsQuery, perhitungan `startup_cmd`)

**Interfaces:**
- Produces: query `path` di `/ws/terminal`; `pub fn resolve_startup_command(bookmark: Option<&Bookmark>, server_id: i64, path: Option<&str>) -> Option<String>` (mengembalikan perintah termasuk `\r`).

- [ ] Step 1: Unit test di `#[cfg(test)] mod tests` terminal_ws.rs:
  - bookmark server sama + path → perintah bookmark (`cd -- /srv/a && clear\r`)
  - bookmark server lain, path `None` → `None`
  - tanpa bookmark, path `/var/www/my app` → `cd -- '/var/www/my app' && clear\r`
  - path `"  "` → `None`
- [ ] Step 2: `cargo test --lib resolve_startup` → FAIL (fungsi belum ada)
- [ ] Step 3: Implement; `WsQuery.path: Option<String>`; ganti blok `startup_cmd`.
- [ ] Step 4: `cargo test --lib resolve_startup` → PASS; `cargo build`.

### Task 2: Frontend — logika murni

**Files:**
- Create: `frontend/src/fileBookmarks.ts`
- Test: `frontend/tests/fileBookmarks.test.ts`

**Interfaces:**
- `normalizeBookmarkPath(path: string): string` — buang trailing `/` kecuali `/` (sama dengan backend).
- `fileBookmarkMap(bookmarks: Bookmark[]): Record<string, string[]>` — key `String(server_id)`, path ter-normalisasi, urut, unik.
- `findPathBookmark(bookmarks: Bookmark[], serverId: number, path: string): Bookmark | undefined`.
- `folderTitle(path: string): string` — `/` → `Root (/)`, selain itu segmen terakhir (fallback path).

- [ ] Step 1: Test: map dua server; `/srv/a/` cocok dengan bookmark `/srv/a`; server lain tidak cocok; `folderTitle("/")`, `folderTitle("/var/www")` = `www`.
- [ ] Step 2: `node --test tests/fileBookmarks.test.ts` → FAIL
- [ ] Step 3: Implement.
- [ ] Step 4: Test → PASS.

### Task 3: Frontend — wiring

**Files:**
- Modify: `frontend/src/types.ts` (`Tab.startPath?: string`)
- Modify: `frontend/src/api.ts` (`terminalSocketUrl(serverId, bookmarkId, cols, rows, startPath?)` → set `path` hanya jika `bookmarkId === null` dan startPath ada)
- Modify: `frontend/src/components/TerminalView.tsx` (teruskan `tab.startPath`)
- Modify: `frontend/src/App.tsx` (`openTerminalAtPath(serverId, path)`; FilesView props `bookmarks`, `onToggleBookmark`)
- Modify: `frontend/src/components/files/FilesView.tsx` (hapus state/localStorage/sync; terima props; hapus key `td_files_bookmarks` saat mount)

- [ ] Step 1: Implement sesuai Interfaces Task 2.
- [ ] Step 2: `npx tsc --noEmit` + `npm test` → PASS; `cargo test` → PASS.
- [ ] Step 3: Commit.
