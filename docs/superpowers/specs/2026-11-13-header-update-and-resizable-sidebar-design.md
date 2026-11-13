# Header Update Notification & Resizable Sidebar Design

## Intent & Why
1. Memberi visibilitas langsung di header kanan (`TabBar`) saat ada rilis baru RusTerm tanpa harus membuka modal update secara manual.
2. Memungkinkan pengguna mengatur lebar sidebar dengan menggeser garis batas kanan seperti file explorer, serta otomatis masuk ke mode mini jika digeser terlalu sempit.

## Goals & Non-Goals
- **Goals:**
  - Cek pembaruan di latar belakang saat sesi aktif (`api.checkUpdate()`) dan tampilkan tombol badge `update` di ujung kanan `.tabbar` bila `has_update === true`.
  - Klik tombol `update` membuka `UpdateModal`.
  - Garis batas kanan `.sidebar` dapat di-drag (mode desktop, non-mini) untuk mengubah lebar (`240px`–`600px`, default `420px`, persisten di `localStorage` `rusterm.sidebarWidth`).
  - Menggeser di bawah ambang `< 160px` otomatis mengaktifkan `sidebarMini = true` (`rusterm.sidebarMini`), sementara lebar terakhir tetap tersimpan untuk saat mode mini dimatikan kembali.
- **Non-Goals:**
  - Mengubah endpoint backend `/api/system/update-check` atau menambah polling periodik agresif.
  - Resize sidebar pada tampilan mobile drawer (`<= 760px`).

## Approaches Considered
- **Dipilih:**
  - State `updateInfo` di `App.tsx` yang diisi saat `token` siap (sekali per sesi / saat modal update ditutup), diteruskan ke `TabBar.tsx`.
  - Pure helper di `frontend/src/sidebarResize.ts` untuk kalkulasi clamp lebar sidebar + deteksi auto-mini (`resolveSidebarDrag`), serta CSS `--sidebar-w` + `.sidebar-resizer` di `App.tsx` & `styles.css`.
- **Ditolak:**
  - Polling berkala ke `/api/system/update-check` (berisiko terkena GitHub API rate limit).
  - Library eksternal seperti `react-resizable-panels` (tidak perlu karena layout sudah berbasis CSS Grid `--sidebar-w`).
