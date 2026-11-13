# Semver Update Check, Idle Auto-Lock & Terminal Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task inline. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement semver version comparison for update checks, configurable idle auto-lock, and in-terminal buffer search (`Ctrl+F`).

**Architecture:** Pure, unit-tested functions handle version comparison in Rust (`system::is_newer_version`), idle timeout calculation in TypeScript (`idleLock.ts`), and buffer text matching in TypeScript (`terminalSearch.ts`), wired into `App.tsx`, `ChangePin.tsx`, and `TerminalView.tsx`.

**Tech Stack:** Rust (Axum), React 18, TypeScript, `@xterm/xterm`, Node test runner.

**Spec:** `docs/superpowers/specs/2026-11-13-semver-autolock-termsearch-design.md`

## Global Constraints
- Zero new npm or Cargo dependencies.
- Every task follows strict TDD (RED test first, watch fail, minimal implementation, watch GREEN).

## Review Focus
- Version strings with leading `v`, pre-release suffixes (`0.2.6-beta.1`), or non-numeric garbage must not panic and must compare safely.
- Idle timer must not trigger when `timeoutMinutes === 0` (disabled) or when `nowMs < lastActiveMs`.
- Empty search queries must return `[]` without infinite loops.

---

### Task 1: Semver Update Comparison & Fallback URL (`backend`)

**Files:**
- Modify: `backend/src/controllers/system.rs`
- Test: `backend/tests/system_test.rs`

**Interfaces:**
- Produces: `pub fn is_newer_version(current: &str, latest: &str) -> bool` in `xterm_backend::controllers::system`

- [ ] **Step 1: Write the failing test in `backend/tests/system_test.rs`**
  Add `test_is_newer_version_semver_comparison` asserting `is_newer_version("0.2.6", "0.2.6") == false`, `is_newer_version("0.2.6", "0.2.5") == false`, `is_newer_version("0.2.6", "0.2.7") == true`, `is_newer_version("0.2.6", "v0.3.0") == true`, `is_newer_version("0.2.6", "invalid") == false`, and update `check.current_version` assertion to `env!("CARGO_PKG_VERSION")`.
- [ ] **Step 2: Run `cargo test --test system_test` and verify RED**
- [ ] **Step 3: Implement `pub fn is_newer_version` and fix non-Windows fallback URL in `backend/src/controllers/system.rs`**
- [ ] **Step 4: Run `cargo test --test system_test` and verify GREEN**

### Task 2: Configurable Idle Auto-Lock (`frontend`)

**Files:**
- Create: `frontend/src/idleLock.ts`
- Modify: `frontend/src/components/ChangePin.tsx`, `frontend/src/App.tsx`
- Test: `frontend/tests/idleLock.test.ts`

**Interfaces:**
- Produces: `IDLE_LOCK_KEY`, `IDLE_LOCK_OPTIONS`, `DEFAULT_IDLE_LOCK_MINUTES`, `IdleLockMinutes`, `normalizeIdleLockMinutes`, `isIdleExpired`, `formatIdleLockLabel` in `frontend/src/idleLock.ts`

- [ ] **Step 1: Write the failing test in `frontend/tests/idleLock.test.ts`**
- [ ] **Step 2: Run `npm test` in `frontend/` and verify RED**
- [ ] **Step 3: Implement `frontend/src/idleLock.ts` and wire into `ChangePin.tsx` and `App.tsx`**
- [ ] **Step 4: Run `npm test` and `npx tsc --noEmit` in `frontend/` and verify GREEN**

### Task 3: In-Terminal Buffer Search (`Ctrl+F`) (`frontend`)

**Files:**
- Create: `frontend/src/terminalSearch.ts`
- Modify: `frontend/src/components/TerminalView.tsx`, `frontend/src/App.tsx`, `frontend/src/styles.css`
- Test: `frontend/tests/terminalSearch.test.ts`

**Interfaces:**
- Produces: `BufferSearchMatch`, `findBufferMatches(lines: readonly string[], query: string): BufferSearchMatch[]`, `stepMatchIndex(current: number, total: number, delta: 1 | -1): number` in `frontend/src/terminalSearch.ts`

- [ ] **Step 1: Write the failing test in `frontend/tests/terminalSearch.test.ts`**
- [ ] **Step 2: Run `npm test` in `frontend/` and verify RED**
- [ ] **Step 3: Implement `frontend/src/terminalSearch.ts` and wire search bar + `Ctrl+F` + pane search button into `TerminalView.tsx`, `App.tsx`, and `styles.css`**
- [ ] **Step 4: Run `npm test`, `npx tsc --noEmit`, and `npm run build` in `frontend/` and verify GREEN**
