#!/bin/bash
#
# Run the LED controller test suite against both Python and Rust implementations
#
# This script:
# 1. Starts the server in debug mode (Python or Rust)
# 2. Waits for the server to be ready
# 3. Runs the test suite
# 4. Outputs results in a format suitable for AI analysis
# 5. Cleans up (kills the server)
#

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
APP_DIR="$PROJECT_ROOT/app"
APP_RS_DIR="$PROJECT_ROOT/app_rs"
APP_VENV="$PROJECT_ROOT/venv"
TESTS_DIR="$PROJECT_ROOT/tests_external"
TESTS_VENV="$TESTS_DIR/venv"
SERVER_LOG="/tmp/led-controller-test-server.log"

SERVER_PID=""
EXIT_CODE=0
RUN_BOTH=false
RUN_RUST=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --rust|-r)
            RUN_RUST=true
            shift
            ;;
        --both|-b)
            RUN_BOTH=true
            shift
            ;;
        --py|--python)
            RUN_RUST=false
            shift
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--rust|-r|--py|--both|-b]"
            echo ""
            echo "Options:"
            echo "  (no args)  Run tests against Python implementation (default)"
            echo "  --py       Same as default"
            echo "  --rust, -r Run tests against Rust implementation"
            echo "  --both, -b Run tests against both implementations"
            exit 1
            ;;
    esac
done

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

# Function to run tests against a specific implementation
run_tests() {
    local impl_name=$1
    local impl_dir=$2
    local start_server_cmd=$3

    echo ""
    echo "========================================"
    echo "Testing $impl_name Implementation"
    echo "========================================"
    echo ""
    echo "Implementation: $impl_name"
    echo "Directory:      $impl_dir"
    echo "Server log:     $SERVER_LOG"
    echo ""

    # Kill any existing server on port 5000
    EXISTING_PID=$(lsof -ti:5000 2>/dev/null || true)
    if [[ -n "$EXISTING_PID" ]]; then
        echo "Killing existing process on port 5000 (PID: $EXISTING_PID)"
        kill $EXISTING_PID 2>/dev/null || true
        sleep 1
    fi

    # Start the server in debug mode
    echo "Starting $impl_name server in debug mode..."
    (
        cd "$impl_dir"
        eval "$start_server_cmd"
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
            return 1
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
        return 1
    fi

    # Verify debug mode is enabled
    DEBUG_MODE=$(curl -s http://localhost:5000/looper | grep -o '"debug_mode":true' || echo "")
    if [[ -z "$DEBUG_MODE" ]]; then
        echo "Error: Server is not in debug mode"
        echo "Server log:"
        cat "$SERVER_LOG"
        return 1
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
    local test_exit_code=$?

    echo ""
    echo "========================================"
    echo "Test Results Summary - $impl_name"
    echo "========================================"
    echo ""

    if [[ $test_exit_code -eq 0 ]]; then
        echo "STATUS: ALL TESTS PASSED ($impl_name)"
    else
        echo "STATUS: SOME TESTS FAILED ($impl_name, exit code: $test_exit_code)"
        echo ""
        echo "Server log ($SERVER_LOG):"
        echo "----------------------------------------"
        tail -50 "$SERVER_LOG"
        echo "----------------------------------------"
    fi

    # Stop the server
    cleanup
    SERVER_PID=""

    return $test_exit_code
}

# Main execution
if [[ "$RUN_BOTH" == "true" ]]; then
    echo "========================================"
    echo "LED Controller Test Suite"
    echo "Running tests against BOTH implementations"
    echo "========================================"

    # Run Python tests
    PY_EXIT=0
    run_tests "Python" "$APP_DIR" "source $APP_VENV/bin/activate && exec python -m __init__ --debug" || PY_EXIT=$?

    # Run Rust tests
    RUST_EXIT=0
    run_tests "Rust" "$APP_RS_DIR" "cargo run -- --debug" || RUST_EXIT=$?

    echo ""
    echo "========================================"
    echo "FINAL SUMMARY"
    echo "========================================"
    echo ""
    if [[ $PY_EXIT -eq 0 && $RUST_EXIT -eq 0 ]]; then
        echo "Python: PASSED"
        echo "Rust:   PASSED"
        echo ""
        echo "STATUS: ALL TESTS PASSED FOR BOTH IMPLEMENTATIONS"
        EXIT_CODE=0
    else
        [[ $PY_EXIT -eq 0 ]] && echo "Python: PASSED" || echo "Python: FAILED (exit code: $PY_EXIT)"
        [[ $RUST_EXIT -eq 0 ]] && echo "Rust:   PASSED" || echo "Rust:   FAILED (exit code: $RUST_EXIT)"
        echo ""
        echo "STATUS: SOME TESTS FAILED"
        EXIT_CODE=1
    fi

else
    # Run single implementation (default to Python)
    if [[ "$RUN_RUST" == "true" ]]; then
        echo "========================================"
        echo "LED Controller Test Suite"
        echo "Testing Rust Implementation"
        echo "========================================"
        run_tests "Rust" "$APP_RS_DIR" "cargo run -- --debug"
        EXIT_CODE=$?
    else
        echo "========================================"
        echo "LED Controller Test Suite"
        echo "Testing Python Implementation"
        echo "========================================"
        run_tests "Python" "$APP_DIR" "source $APP_VENV/bin/activate && exec python -m __init__ --debug"
        EXIT_CODE=$?
    fi
fi

echo ""
exit $EXIT_CODE
