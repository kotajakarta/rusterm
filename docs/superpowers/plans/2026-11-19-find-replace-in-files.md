# Find & Replace in Files Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task inline in the main session. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a modal-based Find & Replace in Files feature to the File module so users can recursively search and replace text across files in a selected folder while excluding folders like `node_modules` and `.git`.

**Architecture:** Backend exposes `POST /api/files/:server_id/search` and `POST /api/files/:server_id/replace` in Rust (Axum), validating auth and paths before invoking new `search` and `replace` operations on the remote Python 3 agent (`files_agent.py`) that already provides `IgnoreRules`, `.gitignore` parsing, binary detection, and atomic file writes. Frontend adds pure helper logic (`fileSearch.ts`), a dedicated `FindReplaceModal.tsx` component with per-file selection and inline replace confirmation, and compact triggers in `FilePane.tsx` (toolbar button, `Ctrl+Shift+F`, and folder/directory context menu).

**Tech Stack:** Rust (Axum, Serde, Tokio), Python 3 stdlib (`files_agent.py`), React 18, TypeScript, Node test runner (`node --test`).

## Global Constraints
- Backend API endpoints must enforce `X-Session` auth via `check_auth` and normalize/validate paths with `sanitize_path`.
- `files_agent.py` must remain Python 3 stdlib-only and preserve file ownership/permissions via atomic write (`_private_tmp_name` + `os.replace`).
- Default excludes must skip common dependency/build/VCS directories (`.git`, `node_modules`, `dist`, `build`, `target`, `.next`, `.nuxt`, `vendor`, `__pycache__`, `.venv`, `venv`, `.idea`, `.vscode`, `coverage`, `.cache`) and respect `.gitignore` by default.
- UI must be a modal (`fm-modal`) so the File Management workspace does not become more cramped.

## Review Focus
1. **Path traversal / symlink escape during replace**: `op_replace` must verify each target file's resolved path stays within `realpath(root)` and is not in `PROTECTED`; symlinks pointing outside `root` must be rejected.
2. **Invalid regex input**: invalid regex in `query` must return a clear `AgentError` (HTTP 400), never crash the agent with an unhandled traceback.
3. **Binary & large files**: files containing NUL bytes in the first 8192 bytes or larger than `MAX_READ_BYTES` (2 MB) must be skipped and counted, never corrupted by replace.
4. **Optimistic concurrency on replace**: if a file's `mtime` changed between search and replace (`expected_mtime`), `op_replace` must skip that file and report it in `errors` rather than silently overwriting external edits.
5. **Result capping**: searches matching thousands of lines must cap at `max_results` (default 500, max 2000) and set `truncated: true` so JSON responses stay fast within the 60s agent timeout.

---

### Task 1: Agent `op_search` and `op_replace` in `files_agent.py`

**Files:**
- Modify: `backend/src/files_agent.py`
- Create: `backend/tests/file_search_agent_test.rs`

**Interfaces:**
- Produces in `files_agent.py`:
  - `op_search(req)` registered in `OPS["search"]`:
    - Input JSON: `{ op: "search", path: str, query: str, case_sensitive?: bool, whole_word?: bool, use_regex?: bool, include?: list[str], exclude?: list[str], use_gitignore?: bool, max_results?: int }`
    - Output JSON: `{ path, files: [{ path, rel_path, mtime, match_count, matches: [{ line, col, text, match_start, match_end }] }], files_scanned, files_matched, total_matches, skipped_binary, skipped_large, truncated }`
  - `op_replace(req)` registered in `OPS["replace"]`:
    - Input JSON: `{ op: "replace", path: str, query: str, replacement: str, case_sensitive?: bool, whole_word?: bool, use_regex?: bool, include?: list[str], exclude?: list[str], use_gitignore?: bool, targets?: list[{ path: str, expected_mtime?: float }] }`
    - Output JSON: `{ path, files_modified, total_replacements, modified_files: [{ path, rel_path, replacements }], errors: [{ path, error }] }`

- [ ] **Step 1: Write failing integration tests in `backend/tests/file_search_agent_test.rs`**
  - Test recursive search finds matches across nested dirs, skips `node_modules` and `.git`, respects `.gitignore`, skips binary files, handles `case_sensitive`, `whole_word`, `use_regex`, `include` glob filter, invalid regex error, and `max_results` truncation.
  - Test replace updates only selected `targets`, preserves permissions, rejects stale `expected_mtime`, rejects symlink/path outside `root`, and supports regex capture groups (`\1`) when `use_regex: true` while treating `$` / `\` literally when `use_regex: false`.
- [ ] **Step 2: Run `cargo test --test file_search_agent_test` and verify FAIL**
- [ ] **Step 3: Implement `op_search` and `op_replace` in `backend/src/files_agent.py`**
- [ ] **Step 4: Run `cargo test --test file_search_agent_test` and verify PASS**

---

### Task 2: Rust Axum Endpoints & Models for Search and Replace

**Files:**
- Modify: `backend/src/models/files.rs`
- Modify: `backend/src/controllers/files.rs`
- Modify: `backend/tests/files_test.rs`

**Interfaces:**
- Produces:
  - `FileSearchBody`, `FileReplaceBody`, `FileReplaceTarget` in `backend/src/models/files.rs`
  - `POST /api/files/:server_id/search` and `POST /api/files/:server_id/replace` in `backend/src/controllers/files.rs`

- [ ] **Step 1: Write failing router tests in `backend/tests/files_test.rs`**
  - Unauthorized request to `/api/files/:id/search` and `/api/files/:id/replace` returns `401`.
  - Empty `query` or relative `path` returns `400`.
  - Non-existent `server_id` with valid auth & payload returns `404`.
- [ ] **Step 2: Run `cargo test --test files_test` and verify FAIL**
- [ ] **Step 3: Add structs in `backend/src/models/files.rs` and handlers/routes in `backend/src/controllers/files.rs`**
- [ ] **Step 4: Run `cargo test --test files_test` and verify PASS**

---

### Task 3: Frontend Search/Replace Pure Helpers & Types + API Client

**Files:**
- Create: `frontend/src/fileSearch.ts`
- Create: `frontend/tests/fileSearch.test.ts`
- Modify: `frontend/src/types.ts`
- Modify: `frontend/src/api.ts`

**Interfaces:**
- Produces in `frontend/src/fileSearch.ts`:
  - `DEFAULT_SEARCH_EXCLUDES: readonly string[]`
  - `parsePatternList(raw: string): string[]`
  - `splitMatchLine(text: string, start: number, end: number): { before: string; match: string; after: string }`
  - `buildReplaceTargets(files: FileSearchFileResult[], selectedPaths: Set<string>): FileReplaceTarget[]`
- Produces in `frontend/src/api.ts`:
  - `api.filesSearch(serverId: number, body: FileSearchBody): Promise<FileSearchOut>`
  - `api.filesReplace(serverId: number, body: FileReplaceBody): Promise<FileReplaceOut>`

- [ ] **Step 1: Write failing unit tests in `frontend/tests/fileSearch.test.ts`**
- [ ] **Step 2: Run `npm test` in `frontend/` and verify FAIL**
- [ ] **Step 3: Implement `frontend/src/fileSearch.ts`, types in `frontend/src/types.ts`, and API methods in `frontend/src/api.ts`**
- [ ] **Step 4: Run `npm test` and `npm run typecheck` in `frontend/` and verify PASS**

---

### Task 4: `FindReplaceModal` Component, Styles, and `FilePane` Integration

**Files:**
- Create: `frontend/src/components/files/FindReplaceModal.tsx`
- Modify: `frontend/src/components/files/FilePane.tsx`
- Modify: `frontend/src/styles.css`

**Interfaces:**
- Produces:
  - `FindReplaceModal` component props: `{ serverId: number; initialPath: string; onClose: () => void; onOpenFile: (path: string) => void; onReplaced: () => void }`
- Integrates in `FilePane.tsx`:
  - Compact toolbar icon button ("Find & Replace in files (Ctrl+Shift+F)")
  - Keyboard shortcut `Ctrl+Shift+F` when pane is active
  - Context menu entries on directory menu and folder item menu ("Find & Replace in folder…")
  - Clicking a file result opens it in `EditorDialog`

- [ ] **Step 1: Create `FindReplaceModal.tsx`** with Find/Replace inputs, option toggles (`Aa`, `ab`, `.*`), collapsible include/exclude & `.gitignore` filters, grouped per-file results with checkboxes, match highlights, open-in-editor action, and inline replace confirmation.
- [ ] **Step 2: Add modal styles in `frontend/src/styles.css`** matching existing `fm-modal` design tokens.
- [ ] **Step 3: Wire `FindReplaceModal` into `FilePane.tsx`** (toolbar button, `Ctrl+Shift+F`, context menus, open file in editor, refresh after replace).
- [ ] **Step 4: Run full verification**: `cargo test` in `backend/`, `npm run typecheck`, `npm test`, and `npm run build` in `frontend/`.
