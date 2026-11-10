#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

if [ -f "$HOME/.cargo/env" ]; then
    # shellcheck source=/dev/null
    . "$HOME/.cargo/env"
fi

PORT=9095
FOREGROUND=0
NO_BROWSER=1
REBUILD=0

while [ $# -gt 0 ]; do
    case "$1" in
        --port)
            PORT="$2"
            shift 2
            ;;
        --foreground)
            FOREGROUND=1
            shift
            ;;
        --browser|--open-browser)
            NO_BROWSER=0
            shift
            ;;
        --no-browser)
            NO_BROWSER=1
            shift
            ;;
        --rebuild)
            REBUILD=1
            shift
            ;;
        -h|--help)
            echo "Usage: ./run.sh [OPTIONS]"
            echo "Options:"
            echo "  --port <PORT>         Set port (default: 9095)"
            echo "  --foreground          Run in foreground"
            echo "  --browser             Open browser automatically"
            echo "  --no-browser          Do not open browser (default)"
            echo "  --rebuild             Force rebuild of frontend and backend"
            exit 0
            ;;
        *)
            shift
            ;;
    esac
done

echo "==> Memeriksa toolchain..."
if ! command -v cargo >/dev/null 2>&1; then
    echo "Error: cargo tidak ditemukan. Pastikan Rust sudah terinstall." >&2
    exit 1
fi

# Build frontend if needed
if [ "$REBUILD" = 1 ] || [ ! -f "frontend/dist/index.html" ]; then
    echo "==> Membangun frontend React + Vite..."
    (cd frontend && npm run build)
fi

# Build backend
echo "==> Membangun backend Rust..."
(cd backend && cargo build --release)

BIN="$SCRIPT_DIR/backend/target/release/xterm-backend"
DATA_DIR="$HOME/.local/share/xterm-rust"
mkdir -p "$DATA_DIR"

PID_FILE="$DATA_DIR/xterm-$PORT.pid"
LOG_FILE="$DATA_DIR/xterm-$PORT.log"

# Check if already running on port
if command -v lsof >/dev/null 2>&1 && lsof -Pi :"$PORT" -sTCP:LISTEN -t >/dev/null 2>&1; then
    echo "==> Port $PORT sedang digunakan. Menghentikan instance sebelumnya..."
    ./stop.sh --port "$PORT" || true
    sleep 1
fi

echo "==> Menjalankan xterm-rust di port $PORT..."
if [ "$FOREGROUND" = 1 ]; then
    exec "$BIN" --port "$PORT" --data-dir "$DATA_DIR" --dist-dir "$SCRIPT_DIR/frontend/dist"
else
    if command -v setsid >/dev/null 2>&1; then
        setsid -f "$BIN" --port "$PORT" --data-dir "$DATA_DIR" --dist-dir "$SCRIPT_DIR/frontend/dist" < /dev/null > "$LOG_FILE" 2>&1
        sleep 0.5
        PID=$(lsof -ti :"$PORT" 2>/dev/null || true)
    else
        nohup "$BIN" --port "$PORT" --data-dir "$DATA_DIR" --dist-dir "$SCRIPT_DIR/frontend/dist" < /dev/null > "$LOG_FILE" 2>&1 &
        PID=$!
        disown "$PID" 2>/dev/null || true
    fi
    if [ -n "$PID" ]; then
        echo "$PID" > "$PID_FILE"
        echo "==> xterm-rust berjalan di background (PID $PID)"
    else
        echo "==> xterm-rust dijalankan di background"
    fi
    echo "==> Log: $LOG_FILE"
    echo "==> Buka di browser: http://localhost:$PORT"

    if [ "$NO_BROWSER" = 0 ]; then
        if command -v xdg-open >/dev/null 2>&1; then
            xdg-open "http://localhost:$PORT" 2>/dev/null || true
        fi
    fi
fi
