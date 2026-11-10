#!/usr/bin/env bash
set -euo pipefail

PORT=1945

while [ $# -gt 0 ]; do
    case "$1" in
        --port)
            PORT="$2"
            shift 2
            ;;
        -h|--help)
            echo "Usage: ./stop.sh [--port <PORT>]"
            exit 0
            ;;
        *)
            shift
            ;;
    esac
done

DATA_DIR="$HOME/.local/share/xterm-rust"
PID_FILE="$DATA_DIR/xterm-$PORT.pid"

STOPPED=0

if [ -f "$PID_FILE" ]; then
    PID=$(cat "$PID_FILE")
    if kill -0 "$PID" 2>/dev/null; then
        echo "Menghentikan xterm-rust (PID $PID)..."
        kill "$PID" 2>/dev/null || true
        for _ in {1..10}; do
            if ! kill -0 "$PID" 2>/dev/null; then
                break
            fi
            sleep 0.5
        done
        if kill -0 "$PID" 2>/dev/null; then
            kill -9 "$PID" 2>/dev/null || true
        fi
        STOPPED=1
    fi
    rm -f "$PID_FILE"
fi

if command -v lsof >/dev/null 2>&1; then
    PORT_PIDS=$(lsof -ti :"$PORT" 2>/dev/null || true)
    if [ -n "$PORT_PIDS" ]; then
        for p in $PORT_PIDS; do
            if kill -0 "$p" 2>/dev/null; then
                kill "$p" 2>/dev/null || true
                STOPPED=1
            fi
        done
    fi
fi

if [ "$STOPPED" = 1 ]; then
    echo "xterm-rust di port $PORT telah dihentikan."
else
    echo "Tidak ada proses xterm-rust yang berjalan di port $PORT."
fi
