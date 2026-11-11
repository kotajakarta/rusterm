#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

if [ -f "$HOME/.cargo/env" ]; then
    # shellcheck source=/dev/null
    . "$HOME/.cargo/env"
fi

echo "============================================================"
echo "    RusTerm Windows Standalone Executable (.exe) Build    "
echo "============================================================"

# 1. Verify MinGW toolchain
if ! command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
    echo "Error: x86_64-w64-mingw32-gcc tidak ditemukan. Silakan pasang paket mingw64-gcc." >&2
    exit 1
fi

# 2. Verify Rust target
if ! rustup target list | grep -q "x86_64-pc-windows-gnu (installed)"; then
    echo "==> Menambahkan Rust target x86_64-pc-windows-gnu..."
    rustup target add x86_64-pc-windows-gnu
fi

# 3. Build frontend
echo "==> [1/3] Membangun bundle frontend React + Vite..."
(cd frontend && npm run build)

# 4. Build Windows release binary with embedded assets
echo "==> [2/3] Mengkompilasi release binary Windows (.exe)..."
(cd backend && cargo build --release --target x86_64-pc-windows-gnu)

# 5. Output packaging
echo "==> [3/3] Menyiapkan paket rilis..."
mkdir -p dist-release
cp backend/target/x86_64-pc-windows-gnu/release/rusterm.exe dist-release/rusterm.exe
cp backend/target/x86_64-pc-windows-gnu/release/rusterm.exe dist-release/termdeck.exe

echo "==> Berhasil! Binary Windows mandiri tersedia di:"
ls -lh dist-release/rusterm.exe
file dist-release/rusterm.exe
