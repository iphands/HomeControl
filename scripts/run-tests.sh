#!/bin/bash
#
# Run the LED controller test suite
#
# This script:
# 1. Starts the server in debug mode (Python or Rust, or both)
# 2. Waits for the server to be ready
# 3. Runs the test suite
# 4. Outputs results
# 5. Cleans up (kills the server)
#

set -e

# Source common functions
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

# Script-specific variables
SERVER_LOG="/tmp/led-controller-test-server.log"
SERVER_PID=""

# Help text
show_help() {
  show_usage "$0" "<py|python|rs|rust|both|all>" \
    "Run the LED controller test suite against server implementations." \
    "  both, all    Run tests against both implementations"
  exit 0
}

# Cleanup function
cleanup() {
  if [[ -n "$SERVER_PID" ]]; then
    cleanup_server "$SERVER_PID"
    SERVER_PID=""
  fi
}

# Run tests against a specific implementation
# Usage: run_tests_impl
# Requires: IMPL, IMPL_NAME, IMPL_DIR, START_CMD to be set
run_tests_impl() {
  echo ""
  echo "========================================"
  echo "Testing $IMPL_NAME Implementation"
  echo "========================================"
  echo ""
  echo "Implementation: $IMPL_NAME"
  echo "Directory:      $IMPL_DIR"
  echo "Server log:     $SERVER_LOG"
  echo ""

  # Kill any existing server
  kill_port_processes
  sleep 1

  # Check Python venv if using Python
  if [[ "$IMPL" == "python" ]]; then
    check_python_venv || return 1
  fi

  # Start the server in debug mode
  echo "Starting $IMPL_NAME server in debug mode..."
  (
    cd "$IMPL_DIR"
    eval "$START_CMD"
  ) > "$SERVER_LOG" 2>&1 &
  SERVER_PID=$!

  echo "Server PID: $SERVER_PID"

  # Wait for server to be ready
  if ! wait_for_server "$SERVER_PID" 30; then
    echo "Error: Server failed to start"
    echo "Server log:"
    cat "$SERVER_LOG"
    return 1
  fi

  # Verify debug mode is enabled
  if ! check_debug_mode; then
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

  local test_exit_code=0
  (
    source "$TESTS_VENV/bin/activate"
    cd "$TESTS_DIR"
    pytest test_led_controller.py -v --tb=short --no-header
  ) || test_exit_code=$?

  echo ""
  echo "========================================"
  echo "Test Results Summary - $IMPL_NAME"
  echo "========================================"
  echo ""

  if [[ $test_exit_code -eq 0 ]]; then
    echo "STATUS: ALL TESTS PASSED ($IMPL_NAME)"
  else
    echo "STATUS: SOME TESTS FAILED ($IMPL_NAME, exit code: $test_exit_code)"
    echo ""
    echo "Server log ($SERVER_LOG):"
    echo "----------------------------------------"
    tail -50 "$SERVER_LOG"
    echo "----------------------------------------"
  fi

  # Stop the server
  cleanup

  return $test_exit_code
}

# Run tests for both implementations
run_both() {
  echo "========================================"
  echo "LED Controller Test Suite"
  echo "Running tests against BOTH implementations"
  echo "========================================"
  echo ""

  local py_exit=0
  local rust_exit=0

  # Run Python tests
  parse_impl_subcommand "python" --debug
  run_tests_impl || py_exit=$?

  # Run Rust tests
  parse_impl_subcommand "rust" --debug
  run_tests_impl || rust_exit=$?

  echo ""
  echo "========================================"
  echo "FINAL SUMMARY"
  echo "========================================"
  echo ""

  if [[ $py_exit -eq 0 && $rust_exit -eq 0 ]]; then
    echo "Python: PASSED"
    echo "Rust:   PASSED"
    echo ""
    echo "STATUS: ALL TESTS PASSED FOR BOTH IMPLEMENTATIONS"
    return 0
  else
    [[ $py_exit -eq 0 ]] && echo "Python: PASSED" || echo "Python: FAILED (exit code: $py_exit)"
    [[ $rust_exit -eq 0 ]] && echo "Rust:   PASSED" || echo "Rust:   FAILED (exit code: $rust_exit)"
    echo ""
    echo "STATUS: SOME TESTS FAILED"
    return 1
  fi
}

# Main function
main() {
  # Set trap for cleanup
  trap cleanup EXIT

  # Check for help flag
  check_help "$@" && show_help

  # Check tests venv exists
  check_tests_venv || exit 1

  # Get subcommand
  local subcommand="${1:-}"

  # Check if subcommand provided
  if [[ -z "$subcommand" ]]; then
    echo "Error: No subcommand given"
    echo ""
    show_usage "$0" "<py|python|rs|rust|both|all>" \
      "Run the LED controller test suite against server implementations." \
      "  both, all    Run tests against both implementations"
    exit 1
  fi

  # Handle both/all special case
  if [[ "$subcommand" == "both" || "$subcommand" == "all" ]]; then
    run_both
    exit $?
  fi

  # Parse implementation subcommand
  if ! parse_impl_subcommand "$subcommand" --debug; then
    echo ""
    show_usage "$0" "<py|python|rs|rust|both|all>" \
      "Run the LED controller test suite against server implementations." \
      "  both, all    Run tests against both implementations"
    exit 1
  fi

  # Run tests for single implementation
  run_tests_impl
  exit $?
}

# Run main
main "$@"
