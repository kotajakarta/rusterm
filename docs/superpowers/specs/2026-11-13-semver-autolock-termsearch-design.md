# Semver Update Check, Idle Auto-Lock & Terminal Search Design

## Intent & Why
Menindaklanjuti 3 rekomendasi prioritas tertinggi dari audit fitur RusTerm:
1. **Perbandingan versi Semver pada `update-check` (`backend/src/controllers/system.rs`)**: Mencegah positif palsu `has_update: true` ketika versi lokal sama atau lebih baru daripada rilis GitHub, serta memperbaiki fallback URL non-Windows.
2. **Kunci Otomatis saat Idle (`frontend/src/idleLock.ts`, `App.tsx`, `ChangePin.tsx`)**: Mengunci sesi RusTerm secara otomatis (`api.lock()`) setelah tidak ada aktivitas pengguna selama durasi yang dapat diatur (`Nonaktif`, `5`, `15`, `30`, `60` menit; default `30` menit).
3. **Pencarian di Dalam Terminal / `Ctrl+F` (`frontend/src/terminalSearch.ts`, `TerminalView.tsx`, `App.tsx`)**: Memungkinkan pencarian teks di seluruh buffer scrollback terminal aktif dengan navigasi Sebelumnya/Berikutnya (`Enter`/`Shift+Enter`) dan tombol Cari di header panel.

## Goals & Non-Goals
- **Goals:**
  - `is_newer_version(current, latest)` membandingkan segmen numerik semver secara akurat.
  - `idleLock.ts` menyediakan normalisasi opsi durasi dan evaluasi kedaluwarsa idle yang diuji unit test.
  - `terminalSearch.ts` menyediakan pencarian substring case-insensitive lintas baris buffer dan navigasi indeks melingkar (`stepMatchIndex`) yang diuji unit test, terhubung ke `term.select` + `term.scrollToLine` di `TerminalView.tsx`.
- **Non-Goals:**
  - Menambah dependensi npm atau crate baru.
