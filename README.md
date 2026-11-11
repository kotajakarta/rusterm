# XTerm-Rust (TermDeck Port to Rust + React/Vite)

A high-performance, lightweight, and secure web-based multi-server SSH terminal built with:
- **Backend**: Rust (Axum, Tokio, Russh async SSH, Rusqlite, Argon2, AES-256-GCM)
- **Frontend**: React 18, Vite, TypeScript, xterm.js, TailwindCSS

## Fitur Utama

- **Cepat & Ringan**: Async runtime native Rust (Tokio & Axum), latency minimal dan konsumsi RAM sangat rendah.
- **Aman**: Seluruh kredensial SSH (password, private key, passphrase) dienkripsi menggunakan AES-256-GCM dengan kunci KEK yang diturunkan dari master PIN menggunakan Argon2id (Zero-Plaintext).
- **Interactive Terminal**: WebSocket streaming real-time dengan alokasi PTY (`xterm-256color`), handling resize window (`SIGWINCH`), dan dukungan bookmark path langsung.
- **Manajemen Server & Proyek**: CRUD server, grouping, pewarnaan label, bookmark direktori/proyek, dan snippets.

## Menjalankan Aplikasi

Pastikan toolchain Rust (`cargo`) dan Node.js sudah terpasang.

Jalankan:
```bash
./run.sh
```

Aplikasi otomatis melakukan build frontend dan backend, lalu membuka antarmuka di **http://localhost:1945**.

### Opsi CLI:
```bash
./run.sh --port 1945        # Menentukan port spesifik
./run.sh --host 0.0.0.0     # Buka akses dari perangkat lain di jaringan (default hanya 127.0.0.1)
./run.sh --foreground       # Berjalan di foreground terminal
./run.sh --no-browser       # Jangan membuka browser otomatis
./run.sh --rebuild          # Paksa build ulang frontend & backend
```

Secara default RusTerm hanya bisa diakses dari komputer ini (`127.0.0.1`). Dengan `--host 0.0.0.0`
siapa pun di jaringan yang sama bisa membuka halaman login, jadi gunakan PIN yang kuat; setelah 5 PIN
salah berturut-turut login dikunci sementara (30 detik, berlipat ganda hingga 15 menit).

Untuk menghentikan:
```bash
./stop.sh                   # Menghentikan server di port 1945
./stop.sh --port 9100       # Menghentikan server di port lain
```
