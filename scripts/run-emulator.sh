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
# Usage:
#   ./run-emulator.sh py    # Run with Python server (default)
#   ./run-emulator.sh rust  # Run with Rust server
#

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
APP_DIR="$PROJECT_ROOT/app"
APP_RS_DIR="$PROJECT_ROOT/app_rs"
APP_VENV="$PROJECT_ROOT/venv"
TESTS_DIR="$PROJECT_ROOT/tests_external"
TESTS_VENV="$TESTS_DIR/venv"
EMU_DIR="$TESTS_DIR/light_strip_emu"
SERVER_LOG="/tmp/led-controller-emulator-server.log"

# Emulator listens on this port
EMU_PORT=4210

SERVER_PID=""

# Parse implementation argument
IMPL="${1:-py}"  # Default to Python if no argument given
case "$IMPL" in
py|python)
  IMPL_NAME="Python"
  IMPL_DIR="$APP_DIR"
  START_CMD="source $APP_VENV/bin/activate && exec python -m __init__ --debug"
  ;;
rust|rs)
  IMPL_NAME="Rust"
  IMPL_DIR="$APP_RS_DIR"
  START_CMD="cargo run -- --debug"
  ;;
*)
  echo "Error: Unknown implementation '$IMPL'"
  echo ""
  echo "Usage: $0 [py|rust]"
  echo ""
  echo "Arguments:"
  echo "  py    - Run with Python server (default)"
  echo "  rust  - Run with Rust server"
  echo ""
  exit 1
  ;;
esac

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
    echo "Killing process on port 5000 (PID: $PORT_PID)..."
    kill -9 $PORT_PID 2>/dev/null || true
  fi

  echo "Done"
}

trap cleanup EXIT

# Check app venv exists (only needed for Python)
if [[ "$IMPL" == "py" || "$IMPL" == "python" ]]; then
  if [[ ! -d "$APP_VENV" ]]; then
    echo "Error: App virtual environment not found at $APP_VENV"
    echo "Create it with: python -m venv $APP_VENV && source $APP_VENV/bin/activate && pip install -r requirements.txt"
    exit 1
  fi
fi

# Check for tkinter in the venv (emulator needs it for GUI)
if ! "$TESTS_VENV/bin/python" -c "import tkinter" 2>/dev/null; then
  echo "WARNING: tkinter not found in venv. Emulator GUI will fail."
  echo "The server will still start and you can use the web UI at http://localhost:5000"
  echo ""
fi

echo "========================================"
echo "LED Strip Emulator"
echo "Implementation: $IMPL_NAME"
echo "========================================"
echo ""
echo "Project root: $PROJECT_ROOT"
echo "Server dir:   $IMPL_DIR"
echo "Emulator port: $EMU_PORT"
echo "Server log:   $SERVER_LOG"
echo ""

# Kill any existing server on port 5000 more aggressively
echo "Checking for existing servers..."

# Kill by port first
EXISTING_PID=$(lsof -ti:5000 2>/dev/null || true)
if [[ -n "$EXISTING_PID" ]]; then
  echo "Killing existing process on port 5000 (PID: $EXISTING_PID)"
  kill -9 $EXISTING_PID 2>/dev/null || true
fi

# Also kill any Python or Rust server processes that might be lingering
pkill -9 -f "python.*__init__.*--debug" 2>/dev/null || true
pkill -9 -f "homectrl" 2>/dev/null || true

# Wait for port to be fully released
for i in {1..5}; do
  if ! lsof -ti:5000 >/dev/null 2>&1; then
    break
  fi
  echo "Waiting for port 5000 to be released..."
  sleep 1
done

# Final check
if lsof -ti:5000 >/dev/null 2>&1; then
  echo "ERROR: Port 5000 is still in use. Please kill the process manually:"
  lsof -i:5000
  exit 1
fi

# Start the server in debug mode (enables strip configuration API)
# Note: Debug mode doesn't pause the looper - it just enables config endpoints
echo "Starting $IMPL_NAME server in debug mode..."
(
  cd "$IMPL_DIR"
  eval "$START_CMD"
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

  if curl -s http://localhost:5000/api/looper >/dev/null 2>&1; then
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
curl -s -X POST http://localhost:5000/api/strips/1 \
  -H "Content-Type: application/json" \
  -d "{\"hostname\": \"127.0.0.1\", \"port\": $EMU_PORT}" > /dev/null

curl -s -X POST http://localhost:5000/api/strips/2 \
  -H "Content-Type: application/json" \
  -d "{\"hostname\": \"127.0.0.1\", \"port\": $EMU_PORT}" > /dev/null

# Verify configuration
echo "Strip configuration:"
curl -s http://localhost:5000/api/strips | python3 -c "import sys,json; d=json.load(sys.stdin); print('  Strip 1:', d['strips'][0]['hostname'], ':', d['strips'][0]['port']); print('  Strip 2:', d['strips'][1]['hostname'], ':', d['strips'][1]['port'])"

echo ""
echo "========================================"
echo "Use the web UI at http://localhost:5000 to control LEDs"
echo "Close the emulator window or click 'Stop Emulator' to exit"
echo "========================================"
echo ""

# Start the emulator (this blocks until user closes it)
# Use the venv python explicitly to avoid shebang issues
cd "$EMU_DIR"
source venv/bin/activate
"$TESTS_VENV/bin/python" emulator.py --port "$EMU_PORT"

echo ""
echo "Emulator closed."
