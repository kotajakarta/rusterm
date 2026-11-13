# Sidebar Tools Popover Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (inline, per user rule). Steps use checkbox (`- [ ]`) syntax.

**Goal:** Pindahkan Snippets, Cloudflare, Tailscale dari `.sidebar-bottom` ke popover anchored yang dibuka tombol `^` di bawah sidebar.

**Architecture:** Logika murni (validasi tab, hitung posisi) di `toolsTray.ts`. Komponen `ToolsTray.tsx` merender tombol + popover via `createPortal(document.body)`, ketiga panel selalu ter-mount, panel non-aktif `hidden`. `App.tsx` mengganti tiga panel dengan `<ToolsTray>` di baris `.sidebar-tools`.

**Tech Stack:** React 18, TypeScript, Vite, `node --test`.

**Spec:** `docs/superpowers/specs/2026-11-13-sidebar-tools-popover-design.md`

## Global Constraints
- Key localStorage: `rusterm.toolsTab`; nilai valid `snippets` | `cloudflare` | `tailscale`; default `snippets`.
- Popover: `left = sidebarLeft + 8`, `bottom = viewportHeight - buttonTop + 6`, `width = min(380, viewportWidth - 16)`, `maxHeight = min(0.7 * viewportHeight, buttonTop - 16)`, minimum maxHeight 160.
- Tanpa dependensi baru. Tidak mengubah Snippets/CloudflarePanel/TailscalePanel.

## Review Focus
- Klik di dalam dialog panel (Modal) tidak menutup popover → dialog di-render dalam subtree portal; pointerdown check pakai `contains`.
- Esc saat dialog terbuka hanya menutup dialog → Modal pakai capture + stopPropagation; ToolsTray pakai listener bubble.
- Nilai localStorage rusak/tidak dikenal → `parseToolsTab` kembali ke `snippets` (test).
- Viewport sangat sempit/pendek → width & maxHeight di-clamp (test).
- Klik tombol saat terbuka → toggle tutup, bukan tutup-lalu-buka (tombol dikecualikan dari klik-luar).

---

### Task 1: Logika murni `toolsTray.ts`

**Files:** Create `frontend/src/toolsTray.ts`; Test `frontend/tests/toolsTray.test.ts`

**Interfaces (Produces):**
- `export type ToolsTab = "snippets" | "cloudflare" | "tailscale"`
- `export const TOOLS_TABS: readonly ToolsTab[]`, `export const TOOLS_TAB_KEY = "rusterm.toolsTab"`
- `export function parseToolsTab(raw: string | null): ToolsTab`
- `export interface TrayPosition { left: number; bottom: number; width: number; maxHeight: number }`
- `export function computeTrayPosition(anchor: { top: number }, sidebarLeft: number, viewport: { width: number; height: number }): TrayPosition`

- [ ] Step 1: tulis test: `parseToolsTab("cloudflare") === "cloudflare"`, `parseToolsTab(null|"x"|"") === "snippets"`; `computeTrayPosition({top:700},0,{width:1280,height:800})` → `{left:8,bottom:106,width:380,maxHeight:560}`; sempit `({top:500},0,{width:300,height:600})` → `width:284, maxHeight:420`; pendek `({top:100},0,{width:1280,height:200})` → `maxHeight:160`.
- [ ] Step 2: `npm test` → FAIL (module not found).
- [ ] Step 3: implement.
- [ ] Step 4: `npm test` → PASS.

### Task 2: Komponen `ToolsTray.tsx` + CSS + integrasi App

**Files:** Create `frontend/src/components/ToolsTray.tsx`; Modify `frontend/src/App.tsx:982-998`, `frontend/src/styles.css` (tambahan di akhir blok sidebar + mini mode).

**Interfaces:** Consumes Task 1. Props: `{ snippets: ReactNode; cloudflare: ReactNode; tailscale: ReactNode; mini: boolean }` — App meneruskan elemen panel yang sudah ada sebagai node agar ToolsTray tidak tahu props panel.

- [ ] Step 1: ToolsTray: state `open`, `tab` (init `parseToolsTab(readString(TOOLS_TAB_KEY))`), `pos`; tombol `.sidebar-tools-btn` (ikon `up`, label "Tools" disembunyikan di mini, `aria-expanded`, `aria-controls`); portal `.tools-tray` `role="dialog"` `hidden={!open}` berisi tablist + 3 tabpanel (`hidden` jika tidak aktif); reposisi saat open & `resize`; pointerdown di luar tray & tombol → tutup; Esc → tutup + fokus balik ke tombol.
- [ ] Step 2: App: hapus `<Snippets>`, `<CloudflarePanel/>`, `<TailscalePanel>` dari `.sidebar-bottom`; tambah `<div className="sidebar-tools"><ToolsTray .../></div>` setelah `.sidebar-bottom`. `.sidebar-bottom` hanya dirender jika `selectedServer`.
- [ ] Step 3: CSS `.sidebar-tools`, `.sidebar-tools-btn`, `.tools-tray`, `.tools-tray-tabs`, `.tools-tray-tab`, `.tools-tray-body`, mini-mode center.
- [ ] Step 4: `npx tsc --noEmit`, `npm test`, `npm run build` → semua lulus.
