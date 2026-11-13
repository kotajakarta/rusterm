# Sidebar Tools Popover — Design (short)

## Intent
Sidebar terlalu sesak: `Snippets`, `CloudflarePanel`, `TailscalePanel` ditumpuk di `.sidebar-bottom`
(max 45% tinggi) sehingga daftar server/path sempit. Di mode mini, semuanya hilang (`.sidebar-bottom` disembunyikan).
Ketiga menu dipindah ke satu tombol `^` di paling bawah sidebar yang membuka popover.

## Goals
- Satu tombol "Tools" (ikon `up`) di baris paling bawah sidebar; di mode mini hanya ikon.
- Klik → popover anchored, muncul ke atas dari tombol, berisi tab **Snippets | Cloudflare | Tailscale**.
- Tab terakhir diingat di `localStorage` (`rusterm.toolsTab`), default `snippets`.
- Tutup dengan Esc, klik di luar, atau klik tombol lagi. Tidak ada backdrop, terminal tetap terlihat.
- `ServerInfo` tetap tampil di `.sidebar-bottom` (disembunyikan di mode mini seperti sekarang).

## Non-goals / anti-goals
- Tidak mengubah isi/perilaku `Snippets`, `CloudflarePanel`, `TailscalePanel`.
- Tidak boleh fetch ulang API Cloudflare/Tailscale tiap buka popover atau ganti tab.
- Dialog milik panel (akun, route, device, sync) tidak boleh menutup popover atau tertutup olehnya.

## Approaches
- **A (dipilih):** popover React; ketiga panel di-mount sekali sejak app start dan panel non-aktif disembunyikan
  dengan atribut `hidden`. State & perilaku API sama persis dengan sekarang.
- B: mount hanya tab aktif — ditolak, fetch ulang & state pilihan akun hilang.
- C: native Popover API — ditolak, top-layer menimpa dialog panel & light-dismiss menutup popover saat klik di dialog.

## Keputusan teknis (call saya, bisa dikoreksi)
- Popover di-render via `createPortal` ke `document.body` dengan `position: fixed`, posisinya dihitung dari
  `getBoundingClientRect()` tombol. Alasan: `.sidebar` memakai `container-type: inline-size` (layout containment)
  dan `.sidebar-bottom` punya `overflow-y: auto`, keduanya akan memotong popover/dialog di dalamnya.
  Karena panel tetap ter-mount saat popover tertutup, wrapper portal selalu ada; saat tertutup diberi `hidden`.
- Klik-luar: listener `pointerdown` di window; abaikan jika target di dalam wrapper popover (dialog panel di-render
  di dalam subtree yang sama, jadi otomatis termasuk) atau di dalam tombol toggle.
- Esc: `Modal.tsx` sudah menangkap Esc di fase capture + `stopPropagation`, jadi Esc saat dialog terbuka hanya menutup dialog.
- Posisi: `left` = sisi kiri sidebar + 8px, `bottom` = jarak dari tombol ke bawah viewport + 6px,
  lebar `min(380px, 100vw - 16px)`, `max-height: min(70vh, top tombol - 16px)` dan scroll internal.
  Reposisi saat popover dibuka dan saat `resize`.
- Logika murni (validasi tab tersimpan, hitung posisi) di `frontend/src/toolsTray.ts` + test `tests/toolsTray.test.ts`.
- Komponen baru `frontend/src/components/ToolsTray.tsx`; `App.tsx` mengganti tiga panel di `.sidebar-bottom`
  dengan `<ToolsTray>` di baris baru `.sidebar-tools` (di luar `.sidebar-bottom` agar tetap tampil di mode mini).
- Aksesibilitas: tombol `aria-expanded`, `aria-controls`; popover `role="dialog"`; tab memakai `role="tablist"/"tab"/"tabpanel"`.

## Diserahkan ke builder
Detail styling (warna, radius, animasi kecil) mengikuti token yang ada di `styles.css`.
