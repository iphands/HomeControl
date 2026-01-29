#!/bin/bash
#
# Run the LED controller test suite
#
# This script:
# 1. Starts the server in debug mode
# 2. Waits for the server to be ready
# 3. Runs the test suite
# 4. Outputs results in a format suitable for AI analysis
# 5. Cleans up (kills the server)
#

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
APP_DIR="$PROJECT_ROOT/app"
APP_VENV="$PROJECT_ROOT/venv"
TESTS_DIR="$PROJECT_ROOT/tests_external"
TESTS_VENV="$TESTS_DIR/venv"
SERVER_LOG="/tmp/led-controller-test-server.log"

SERVER_PID=""
EXIT_CODE=0

# Cleanup function - always kill the server
cleanup() {
    echo ""
    echo "Cleaning up..."

    # Kill the server process and any children
    if [[ -n "$SERVER_PID" ]]; then
        # Kill the process group
        pkill -P "$SERVER_PID" 2>/dev/null || true
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi

    # Also kill any lingering processes on port 5000
    local PORT_PID=$(lsof -ti:5000 2>/dev/null || true)
    if [[ -n "$PORT_PID" ]]; then
        kill $PORT_PID 2>/dev/null || true
    fi

    echo "Server stopped"
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
echo "LED Controller Test Suite"
echo "========================================"
echo ""
echo "Project root: $PROJECT_ROOT"
echo "App venv:     $APP_VENV"
echo "Tests venv:   $TESTS_VENV"
echo "Server log:   $SERVER_LOG"
echo ""

# Kill any existing server on port 5000
EXISTING_PID=$(lsof -ti:5000 2>/dev/null || true)
if [[ -n "$EXISTING_PID" ]]; then
    echo "Killing existing process on port 5000 (PID: $EXISTING_PID)"
    kill $EXISTING_PID 2>/dev/null || true
    sleep 1
fi

# Start the server in debug mode (redirect output to log file)
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
    # Check if server process is still running
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
        echo " FAILED"
        echo "Error: Server process died"
        echo "Server log:"
        cat "$SERVER_LOG"
        exit 1
    fi

    # Try to connect
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

# Verify debug mode is enabled
DEBUG_MODE=$(curl -s http://localhost:5000/looper | grep -o '"debug_mode":true' || echo "")
if [[ -z "$DEBUG_MODE" ]]; then
    echo "Error: Server is not in debug mode"
    echo "Server log:"
    cat "$SERVER_LOG"
    exit 1
fi
echo "Debug mode: enabled"
echo ""

# Run tests
echo "========================================"
echo "Running Tests"
echo "========================================"
echo ""

(
    source "$TESTS_VENV/bin/activate"
    cd "$TESTS_DIR"

    # Run pytest with verbose output
    # --tb=short for concise tracebacks useful for debugging
    pytest test_led_controller.py \
        -v \
        --tb=short \
        --no-header
)
EXIT_CODE=$?

echo ""
echo "========================================"
echo "Test Results Summary"
echo "========================================"
echo ""

if [[ $EXIT_CODE -eq 0 ]]; then
    echo "STATUS: ALL TESTS PASSED"
else
    echo "STATUS: SOME TESTS FAILED (exit code: $EXIT_CODE)"
    echo ""
    echo "Server log ($SERVER_LOG):"
    echo "----------------------------------------"
    tail -50 "$SERVER_LOG"
    echo "----------------------------------------"
    echo ""
    echo "To debug failures:"
    echo "  1. Start server: cd $APP_DIR && source $APP_VENV/bin/activate && python -m __init__ --debug"
    echo "  2. Run specific test: cd $TESTS_DIR && source $TESTS_VENV/bin/activate && pytest test_led_controller.py -v -k 'test_name'"
fi

echo ""
exit $EXIT_CODE
