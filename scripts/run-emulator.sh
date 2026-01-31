#!/bin/bash
#
# Run the LED Strip Emulator with the Rust server only
#
# This script:
# 1. Starts the Rust server in debug mode (to allow strip configuration)
# 2. Configures strips to send to localhost where the emulator listens
# 3. Starts the LED strip emulator GUI
# 4. Stops the server when the emulator exits
#

set -e

# Source common functions
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

# Script-specific variables
SERVER_LOG="/tmp/led-controller-emulator-server.log"
SERVER_PID=""
EMU_PORT=4210

# Help text
show_help() {
  show_usage "$0" "<rs|rust>" \
    "Run the LED Strip Emulator with the Rust server only." \
    ""
  exit 0
}

# Cleanup function
cleanup() {
  if [[ -n "$SERVER_PID" ]]; then
    cleanup_server "$SERVER_PID"
    SERVER_PID=""
  fi
}

# Configure strips to point to emulator
configure_strips() {
  echo "Configuring strips to send to localhost:$EMU_PORT..."

  curl -s -X POST "http://localhost:$SERVER_PORT/api/strips/1" \
    -H "Content-Type: application/json" \
    -d "{\"hostname\": \"127.0.0.1\", \"port\": $EMU_PORT}" > /dev/null

  curl -s -X POST "http://localhost:$SERVER_PORT/api/strips/2" \
    -H "Content-Type: application/json" \
    -d "{\"hostname\": \"127.0.0.1\", \"port\": $EMU_PORT}" > /dev/null

  echo "Strip configuration:"
  curl -s "http://localhost:$SERVER_PORT/api/strips" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('  Strip 1:', d['strips'][0]['hostname'], ':', d['strips'][0]['port'])
print('  Strip 2:', d['strips'][1]['hostname'], ':', d['strips'][1]['port'])
"
}

# Set mode to RainbowCycle for both strips
set_rainbow_mode() {
  echo "Setting RainbowCycle mode for strips..."

  curl -s -X POST "http://localhost:$SERVER_PORT/api/strips/1/mode" \
    -H "Content-Type: application/json" \
    -d "{\"mode\": \"RainbowCycle\"}" > /dev/null

  curl -s -X POST "http://localhost:$SERVER_PORT/api/strips/2/mode" \
    -H "Content-Type: application/json" \
    -d "{\"mode\": \"RainbowCycle\"}" > /dev/null

  echo "Mode set to RainbowCycle for both strips"
}

# Main function
main() {
  # Set trap for cleanup
  trap cleanup EXIT

  # Check for help flag
  check_help "$@" && show_help

  # Parse subcommand - only support Rust for this version
  if ! parse_impl_subcommand "$1" --debug; then
    echo ""
    show_usage "$0" "<rs|rust>" \
      "Run the LED Strip Emulator with the Rust server only."
    exit 1
  fi

  # Only support Rust implementation
  if [[ "$IMPL" != "rust" ]]; then
    echo "Error: This script only supports Rust implementation"
    echo "Usage: $0 rust"
    exit 1
  fi

  # Display startup info
  echo "========================================"
  echo "LED Strip Emulator (Rust only)"
  echo "Implementation: $IMPL_NAME"
  echo "========================================"
  echo ""
  echo "Project root: $PROJECT_ROOT"
  echo "Server dir:   $IMPL_DIR"
  echo "Emulator port: $EMU_PORT"
  echo "Server log:   $SERVER_LOG"
  echo ""

  # Kill any existing server
  echo "Checking for existing servers..."
  kill_port_processes
  if ! wait_for_port_release; then
    echo "ERROR: Port $SERVER_PORT is still in use. Please kill the process manually:"
    lsof -i:"$SERVER_PORT"
    exit 1
  fi

  # Start the Rust server in debug mode
  echo "Starting Rust server in debug mode..."
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
    exit 1
  fi

  # Configure strips to point to emulator
  configure_strips

  # Set RainbowCycle mode
  set_rainbow_mode

  echo ""
  echo "========================================"
  echo "Use the web UI at http://localhost:$SERVER_PORT to control LEDs"
  echo "Close the emulator window or click 'Stop Server' to exit"
  echo "========================================"
  echo ""

  # Build and run the Rust emulator
  echo "Building and running Rust emulator..."
  cd "$PROJECT_ROOT/emu"
  if ! cargo build; then
    echo "Error: Failed to build Rust emulator"
    exit 1
  fi

  # Run the emulator
  ./target/debug/homectrl-emulator

  echo ""
  echo "Emulator closed."
}

# Run main
main "$@"