#!/bin/bash
#
# Run the LED Strip Emulator with the server
#
# This script:
# 1. Starts the server in debug mode (to allow strip configuration)
# 2. Configures strips to send to localhost where the emulator listens
# 3. Starts the LED strip emulator GUI
# 4. Stops the server when the emulator exits
#

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
APP_DIR="$PROJECT_ROOT/app"
APP_VENV="$PROJECT_ROOT/venv"
TESTS_DIR="$PROJECT_ROOT/tests_external"
TESTS_VENV="$TESTS_DIR/venv"
EMU_DIR="$TESTS_DIR/light_strip_emu"
SERVER_LOG="/tmp/led-controller-emulator-server.log"

# Emulator listens on this port
EMU_PORT=4210

SERVER_PID=""

# Cleanup function
cleanup() {
    echo ""
    echo "Cleaning up..."

    # Kill server if running
    if [[ -n "$SERVER_PID" ]] && kill -0 "$SERVER_PID" 2>/dev/null; then
        echo "Stopping server (PID: $SERVER_PID)..."
        pkill -P "$SERVER_PID" 2>/dev/null || true
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi

    # Kill any lingering processes on port 5000
    local PORT_PID=$(lsof -ti:5000 2>/dev/null || true)
    if [[ -n "$PORT_PID" ]]; then
        kill $PORT_PID 2>/dev/null || true
    fi

    echo "Done"
}

trap cleanup EXIT

# Check app venv exists
if [[ ! -d "$APP_VENV" ]]; then
    echo "Error: App virtual environment not found at $APP_VENV"
    echo "Create it with: python -m venv $APP_VENV && source $APP_VENV/bin/activate && pip install -r requirements.txt"
    exit 1
fi

# Check/create test venv
if [[ ! -d "$TESTS_VENV" ]]; then
    echo "Creating test virtual environment..."
    python -m venv "$TESTS_VENV"
    source "$TESTS_VENV/bin/activate"
    pip install -q -r "$TESTS_DIR/requirements.txt"
    deactivate
fi

echo "========================================"
echo "LED Strip Emulator"
echo "========================================"
echo ""
echo "Project root: $PROJECT_ROOT"
echo "Emulator port: $EMU_PORT"
echo "Server log:   $SERVER_LOG"
echo ""

# Kill any existing server on port 5000
EXISTING_PID=$(lsof -ti:5000 2>/dev/null || true)
if [[ -n "$EXISTING_PID" ]]; then
    echo "Killing existing process on port 5000 (PID: $EXISTING_PID)"
    kill $EXISTING_PID 2>/dev/null || true
    sleep 1
fi

# Start the server in debug mode (enables strip configuration API)
# Note: Debug mode doesn't pause the looper - it just enables config endpoints
echo "Starting server in debug mode..."
(
    cd "$APP_DIR"
    source "$APP_VENV/bin/activate"
    exec python -m __init__ --debug
) > "$SERVER_LOG" 2>&1 &
SERVER_PID=$!

echo "Server PID: $SERVER_PID"

# Wait for server to be ready
echo -n "Waiting for server to start"
MAX_WAIT=30
WAITED=0
while [[ $WAITED -lt $MAX_WAIT ]]; do
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
        echo " FAILED"
        echo "Error: Server process died"
        echo "Server log:"
        cat "$SERVER_LOG"
        exit 1
    fi

    if curl -s http://localhost:5000/looper >/dev/null 2>&1; then
        echo " ready!"
        break
    fi
    echo -n "."
    sleep 1
    ((WAITED++))
done

if [[ $WAITED -ge $MAX_WAIT ]]; then
    echo " TIMEOUT"
    echo "Error: Server did not start within ${MAX_WAIT}s"
    echo "Server log:"
    cat "$SERVER_LOG"
    exit 1
fi

# Configure strips to send to localhost (where emulator listens)
echo "Configuring strips to send to localhost:$EMU_PORT..."
curl -s -X POST http://localhost:5000/strips/1 \
    -H "Content-Type: application/json" \
    -d "{\"hostname\": \"127.0.0.1\", \"port\": $EMU_PORT}" > /dev/null

curl -s -X POST http://localhost:5000/strips/2 \
    -H "Content-Type: application/json" \
    -d "{\"hostname\": \"127.0.0.1\", \"port\": $EMU_PORT}" > /dev/null

# Verify configuration
echo "Strip configuration:"
curl -s http://localhost:5000/strips | python3 -c "import sys,json; d=json.load(sys.stdin); print('  Strip 1:', d['strips'][0]['hostname'], ':', d['strips'][0]['port']); print('  Strip 2:', d['strips'][1]['hostname'], ':', d['strips'][1]['port'])"

echo ""
echo "========================================"
echo "Use the web UI at http://localhost:5000 to control LEDs"
echo "Close the emulator window or click 'Stop Emulator' to exit"
echo "========================================"
echo ""

# Start the emulator (this blocks until user closes it)
(
    source "$TESTS_VENV/bin/activate"
    cd "$EMU_DIR"
    python emulator.py --port "$EMU_PORT"
)

echo ""
echo "Emulator closed."
