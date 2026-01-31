#!/bin/bash
# Run the LED emulator with the server
# Usage: ./scripts/run-emu.sh [python|rust]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

SERVER_TYPE="${1:-python}"
SERVER_PID=""

cleanup() {
    echo "Stopping server..."
    if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
    echo "Done."
}

trap cleanup EXIT INT TERM

# Build the emulator first
echo "Building LED emulator..."
cd "$PROJECT_DIR/emu"
cargo build --release

# Start the appropriate server
cd "$PROJECT_DIR"

case "$SERVER_TYPE" in
    python|py)
        echo "Starting Python server in debug mode..."
        source "$PROJECT_DIR/venv/bin/activate"
        python -m app --debug &
        SERVER_PID=$!
        ;;
    rust|rs)
        echo "Starting Rust server in debug mode..."
        cd "$PROJECT_DIR/app_rs"
        cargo run --release -- --debug &
        SERVER_PID=$!
        cd "$PROJECT_DIR"
        ;;
    *)
        echo "Unknown server type: $SERVER_TYPE"
        echo "Usage: $0 [python|rust]"
        exit 1
        ;;
esac

# Wait a moment for server to start
echo "Waiting for server to start..."
sleep 2

# Check if server is running
if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    echo "Server failed to start!"
    exit 1
fi

# Run the emulator (it will block until closed)
echo "Starting LED emulator..."
echo "Close the emulator window to stop both emulator and server."
"$PROJECT_DIR/emu/target/release/led-emu" --port 4210

# Cleanup happens via trap
