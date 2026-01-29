#!/bin/bash
#
# Common functions and variables for HomeCtrl scripts
# Source this file at the top of each script
#

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Common paths
APP_DIR="$PROJECT_ROOT/app"
APP_RS_DIR="$PROJECT_ROOT/app_rs"
APP_VENV="$PROJECT_ROOT/venv"
TESTS_DIR="$PROJECT_ROOT/tests_external"
TESTS_VENV="$TESTS_DIR/venv"

# Server port
SERVER_PORT=5000

# Display usage message and exit
# Usage: show_usage "$0" "py|rust" "Description of what the script does"
show_usage() {
  local script_name="$1"
  local subcommands="$2"
  local description="$3"
  local extra_info="${4:-}"

  echo "$description"
  echo ""
  echo "Usage: $(basename "$script_name") $subcommands"
  echo ""
  echo "Subcommands:"
  echo "  py, python   Use Python implementation"
  echo "  rs, rust     Use Rust implementation"
  [[ -n "$extra_info" ]] && echo "$extra_info"
  echo ""
  echo "Options:"
  echo "  --help       Show this help message"
}

# Check if help flag was passed
# Usage: check_help "$@" && show_usage ...
check_help() {
  for arg in "$@"; do
    if [[ "$arg" == "--help" || "$arg" == "-h" ]]; then
      return 0
    fi
  done
  return 1
}

# Parse implementation subcommand
# Sets IMPL, IMPL_NAME, IMPL_DIR, and START_CMD variables
# Usage: parse_impl_subcommand "$1" [--debug]
# Returns 0 on success, 1 on failure (will print error)
parse_impl_subcommand() {
  local impl="$1"
  local debug_flag="${2:-}"

  if [[ -z "$impl" ]]; then
    echo "Error: No subcommand given"
    return 1
  fi

  case "$impl" in
  py|python)
    IMPL="python"
    IMPL_NAME="Python"
    IMPL_DIR="$APP_DIR"
    if [[ -n "$debug_flag" ]]; then
      START_CMD="source $APP_VENV/bin/activate && exec python -m __init__ --debug"
    else
      START_CMD="source $APP_VENV/bin/activate && exec python -m __init__"
    fi
    return 0
    ;;
  rs|rust)
    IMPL="rust"
    IMPL_NAME="Rust"
    IMPL_DIR="$APP_RS_DIR"
    if [[ -n "$debug_flag" ]]; then
      START_CMD="cargo run -- --debug"
    else
      START_CMD="cargo run"
    fi
    return 0
    ;;
  --help|-h)
    return 2  # Special return code for help
    ;;
  *)
    echo "Error: Unknown subcommand '$impl'"
    return 1
    ;;
  esac
}

# Check if Python virtual environment exists
# Usage: check_python_venv
check_python_venv() {
  if [[ ! -d "$APP_VENV" ]]; then
    echo "Error: Python virtual environment not found at $APP_VENV"
    echo "Create it with: python -m venv $APP_VENV && source $APP_VENV/bin/activate && pip install -r requirements.txt"
    return 1
  fi
  return 0
}

# Check if test virtual environment exists
# Usage: check_tests_venv
check_tests_venv() {
  if [[ ! -d "$TESTS_VENV" ]]; then
    echo "Error: Tests virtual environment not found at $TESTS_VENV"
    echo "Create it with: cd $TESTS_DIR && python -m venv venv && source venv/bin/activate && pip install -r requirements.txt"
    return 1
  fi
  return 0
}

# Kill any process using the server port
# Usage: kill_port_processs [port]
kill_port_processes() {
  local port="${1:-$SERVER_PORT}"
  local port_pid

  port_pid=$(lsof -ti:"$port" 2>/dev/null || true)
  if [[ -n "$port_pid" ]]; then
    echo "Killing existing process on port $port (PID: $port_pid)"
    kill -9 "$port_pid" 2>/dev/null || true
  fi

  # Also kill any lingering server processes
  pkill -9 -f "python.*__init__.*--debug" 2>/dev/null || true
  pkill -9 -f "homectrl" 2>/dev/null || true
}

# Wait for port to be released
# Usage: wait_for_port_release [port] [max_attempts]
wait_for_port_release() {
  local port="${1:-$SERVER_PORT}"
  local max_attempts="${2:-5}"

  for ((i=1; i<=max_attempts; i++)); do
    if ! lsof -ti:"$port" >/dev/null 2>&1; then
      return 0
    fi
    echo "Waiting for port $port to be released..."
    sleep 1
  done

  return 1
}

# Wait for server to be ready
# Usage: wait_for_server [pid] [max_wait_seconds]
# Returns 0 if ready, 1 if failed, 2 if timeout
wait_for_server() {
  local server_pid="$1"
  local max_wait="${2:-30}"
  local waited=0

  echo -n "Waiting for server to start"
  while ((waited < max_wait)); do
    # Check if server process is still running
    if ! kill -0 "$server_pid" 2>/dev/null; then
      echo " FAILED"
      return 1
    fi

    # Try to connect
    if curl -s "http://localhost:$SERVER_PORT/api/looper" >/dev/null 2>&1; then
      echo " ready!"
      return 0
    fi

    echo -n "."
    sleep 1
    ((waited++))
  done

  echo " TIMEOUT"
  return 2
}

# Cleanup function for trap
# Usage: cleanup [server_pid]
cleanup_server() {
  local server_pid="${1:-}"

  echo ""
  echo "Cleaning up..."

  if [[ -n "$server_pid" ]] && kill -0 "$server_pid" 2>/dev/null; then
    echo "Stopping server (PID: $server_pid)..."
    pkill -P "$server_pid" 2>/dev/null || true
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi

  kill_port_processes
  echo "Done"
}

# Verify server is running in debug mode
# Usage: check_debug_mode
# Returns 0 if debug mode enabled, 1 otherwise
check_debug_mode() {
  local debug_mode
  debug_mode=$(curl -s "http://localhost:$SERVER_PORT/api/looper" | grep -o '"debug_mode":true' || echo "")
  if [[ -z "$debug_mode" ]]; then
    return 1
  fi
  return 0
}
