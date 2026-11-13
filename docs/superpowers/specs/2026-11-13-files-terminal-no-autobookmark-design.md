# File manager: Terminal tanpa auto-bookmark & sinkron bookmark ↔ sidebar

## Intent
Klik **Terminal** di file manager hanya membuka terminal di folder itu, tanpa menambah path
ke sidebar. Path masuk sidebar hanya jika folder di-bookmark. Bookmark file manager dan path
sidebar selalu sama (hapus di salah satu → hilang di keduanya).

## Kondisi awal
- `App.tsx` `onOpenTerminal` dari `FilesView` memanggil `openOrCreatePresetPath` → membuat bookmark.
- Bookmark file manager = tabel `bookmarks` yang sama dengan sidebar (`db.rs` `toggle_file_bookmark`).
- `FilesView` menyimpan cache `localStorage` (`td_files_bookmarks`) lalu `syncFileBookmarks`
  menambahkan ulang path yang sudah dihapus dari sidebar. Toggle di file manager tidak
  memperbarui state `bookmarks` di App.
- `/ws/terminal` hanya bisa `cd` lewat `bookmark_id`.

## Approaches
- **A (dipilih):** query param `path` di `/ws/terminal`; backend mengetik `build_cd_command(path, "")`
  lewat `StartupGate` yang sudah ada. Quoting aman, menunggu prompt.
- B: frontend kirim `cd` setelah `connected` — rapuh (input sebelum prompt bisa hilang).

## Design
- Backend: `WsQuery.path: Option<String>`. Fungsi murni `resolve_startup_command(bookmark, server_id, path)`:
  bookmark milik server ini menang; jika tidak ada, `path` non-kosong → `cd`. Tidak ada `startup_cmd`
  dari query (hanya `cd`, path di-quote).
- Frontend:
  - `Tab.startPath?: string`; `terminalSocketUrl` menerima `startPath` dan set `path` bila tanpa bookmark.
  - Klik Terminal di file manager: jika ada bookmark server+path yang sama → `openTab(server, id, false)`;
    jika tidak → tab baru dengan `startPath`, judul `server: folder`. Tidak membuat bookmark.
  - `FilesView` menerima `bookmarks` (Bookmark[]) dari App dan `onToggleBookmark`; peta per server
    diturunkan oleh fungsi murni `fileBookmarkMap`. localStorage `td_files_bookmarks` dan
    `syncFileBookmarks` tidak dipakai lagi (key lama dihapus sekali saat mount).
  - Toggle: panggil `api.toggleFileBookmark` lalu `reload()` App → sidebar & file manager ikut berubah.
- Non-goal: preset di Home tetap membuat bookmark; endpoint `/files/bookmarks/sync` dibiarkan ada.
