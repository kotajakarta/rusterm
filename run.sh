#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

if [ -f "$HOME/.cargo/env" ]; then
    # shellcheck source=/dev/null
    . "$HOME/.cargo/env"
fi

PORT=1945
HOST=127.0.0.1
FOREGROUND=0
NO_BROWSER=0
APP_MODE=1
REBUILD=0

while [ $# -gt 0 ]; do
    case "$1" in
        --port)
            PORT="$2"
            shift 2
            ;;
        --host)
            HOST="$2"
            shift 2
            ;;
        --foreground)
            FOREGROUND=1
            shift
            ;;
        --no-app)
            APP_MODE=0
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
            echo "  --port <PORT>         Set port (default: 1945)"
            echo "  --host <ADDR>         Bind address (default: 127.0.0.1; 0.0.0.0 opens it to the LAN)"
            echo "  --foreground          Run in foreground"
            echo "  --no-app              Open as standard browser tab instead of app window"
            echo "  --no-browser          Do not open browser"
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

open_browser() {
    local target_port="$1"
    local use_app="$2"
    local url="http://localhost:$target_port"

    if [ "$use_app" = 1 ]; then
        # 1. Check WSL environment & Windows browsers
        if [ -f /proc/version ] && grep -qi microsoft /proc/version; then
            local win_browsers=(
                "/mnt/c/Program Files/Google/Chrome/Application/chrome.exe"
                "/mnt/c/Program Files (x86)/Google/Chrome/Application/chrome.exe"
                "/mnt/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"
                "/mnt/c/Program Files/Microsoft/Edge/Application/msedge.exe"
                "/mnt/c/Program Files/BraveSoftware/Brave-Browser/Application/brave.exe"
            )
            local win_app_browser=""
            for b in "${win_browsers[@]}"; do
                if [ -f "$b" ]; then
                    win_app_browser="$b"
                    break
                fi
            done

            if [ -n "$win_app_browser" ]; then
                local win_localappdata
                win_localappdata=$(cmd.exe /c "echo %LOCALAPPDATA%" 2>/dev/null | tr -d '\r')
                if [ -n "$win_localappdata" ]; then
                    local win_profile="${win_localappdata}\\xterm-rust\\browser"
                    echo "==> Membuka TermDeck dalam mode aplikasi mandiri (App Window)..."
                    "$win_app_browser" --app="$url" --user-data-dir="$win_profile" --no-first-run --no-default-browser-check >/dev/null 2>&1 &
                    return 0
                fi
            fi
        fi

        # 2. Native Linux Chromium-based browsers
        local linux_browsers=(
            "google-chrome"
            "google-chrome-stable"
            "chromium"
            "chromium-browser"
            "brave-browser"
            "microsoft-edge"
            "microsoft-edge-stable"
        )
        for b in "${linux_browsers[@]}"; do
            if command -v "$b" >/dev/null 2>&1; then
                echo "==> Membuka TermDeck dalam mode aplikasi mandiri ($b --app)..."
                local linux_profile="$DATA_DIR/app-browser"
                mkdir -p "$linux_profile"
                "$b" --app="$url" --user-data-dir="$linux_profile" --no-first-run --no-default-browser-check >/dev/null 2>&1 &
                return 0
            fi
        done
    fi

    # 3. Fallback or --no-app: open as standard tab
    echo "==> Membuka di browser default ($url)..."
    if command -v wslview >/dev/null 2>&1; then
        wslview "$url" >/dev/null 2>&1 || true
    elif command -v xdg-open >/dev/null 2>&1; then
        xdg-open "$url" >/dev/null 2>&1 || true
    elif command -v cmd.exe >/dev/null 2>&1; then
        cmd.exe /c start "$url" >/dev/null 2>&1 || true
    fi
}

# Check if already running on port
if command -v lsof >/dev/null 2>&1 && lsof -Pi :"$PORT" -sTCP:LISTEN -t >/dev/null 2>&1; then
    echo "==> Port $PORT sedang digunakan. Menghentikan instance sebelumnya..."
    ./stop.sh --port "$PORT" || true
    sleep 1
fi

echo "==> Menjalankan xterm-rust di port $PORT..."
if [ "$FOREGROUND" = 1 ]; then
    if [ "$NO_BROWSER" = 0 ]; then
        (
            for _ in {1..20}; do
                if command -v lsof >/dev/null 2>&1 && lsof -Pi :"$PORT" -sTCP:LISTEN -t >/dev/null 2>&1; then
                    break
                fi
                sleep 0.25
            done
            open_browser "$PORT" "$APP_MODE"
        ) &
    fi
    exec "$BIN" --port "$PORT" --host "$HOST" --data-dir "$DATA_DIR" --dist-dir "$SCRIPT_DIR/frontend/dist"
else
    if command -v setsid >/dev/null 2>&1; then
        setsid -f "$BIN" --port "$PORT" --host "$HOST" --data-dir "$DATA_DIR" --dist-dir "$SCRIPT_DIR/frontend/dist" < /dev/null > "$LOG_FILE" 2>&1
        sleep 0.5
        PID=$(lsof -ti :"$PORT" 2>/dev/null || true)
    else
        nohup "$BIN" --port "$PORT" --host "$HOST" --data-dir "$DATA_DIR" --dist-dir "$SCRIPT_DIR/frontend/dist" < /dev/null > "$LOG_FILE" 2>&1 &
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
    echo "==> URL: http://localhost:$PORT"

    if [ "$NO_BROWSER" = 0 ]; then
        for _ in {1..20}; do
            if command -v lsof >/dev/null 2>&1 && lsof -Pi :"$PORT" -sTCP:LISTEN -t >/dev/null 2>&1; then
                break
            fi
            sleep 0.25
        done
        open_browser "$PORT" "$APP_MODE"
    fi
fi
