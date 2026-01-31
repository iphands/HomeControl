#!/bin/bash
#
# Run the LED Strip Emulator with the Rust server
#
# This script:
# 1. Starts the Rust server in debug mode (to allow strip configuration)
# 2. Configures strips to send to localhost where the emulator listens
# 3. Starts the Rust LED strip emulator GUI
# 4. Stops the server when the emulator exits
#

set -e

# Source common functions
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

# Script-specific variables
EMU_DIR="$PROJECT_ROOT/emu"
SERVER_LOG="/tmp/led-controller-emulator-server.log"
SERVER_PID=""
EMU_PORT=4210
IMPL="rust"
IMPL_NAME="Rust"
IMPL_DIR="$APP_RS_DIR"
START_CMD="cargo run -- --debug"

# Help text
show_help() {
  echo "Run the LED Strip Emulator with the Rust server."
  echo ""
  echo "Usage: $(basename "$0")"
  echo ""
  echo "Options:"
  echo "  --help       Show this help message"
  echo ""
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
  local response
  response=$(curl -s "http://localhost:$SERVER_PORT/api/strips")
  # Extract strip info using grep/sed (basic JSON parsing)
  echo "  Strip 1:" $(echo "$response" | grep -o '"hostname":"[^"]*"' | head -1 | cut -d'"' -f4) ":" $(echo "$response" | grep -o '"port":[0-9]*' | head -1 | cut -d':' -f2)
  echo "  Strip 2:" $(echo "$response" | grep -o '"hostname":"[^"]*"' | tail -1 | cut -d'"' -f4) ":" $(echo "$response" | grep -o '"port":[0-9]*' | tail -1 | cut -d':' -f2)
}

# Set mode to RainbowColor for both strips
set_rainbow_mode() {
  echo "Setting RainbowColor mode for strips..."

  curl -s -X POST "http://localhost:$SERVER_PORT/api/strips/1/mode" \
    -H "Content-Type: application/json" \
    -d "{\"mode\": \"RainbowColor\"}" > /dev/null

  curl -s -X POST "http://localhost:$SERVER_PORT/api/strips/2/mode" \
    -H "Content-Type: application/json" \
    -d "{\"mode\": \"RainbowColor\"}" > /dev/null

  echo "Mode set to RainbowColor for both strips"
}

# Main function
main() {
  # Set trap for cleanup
  trap cleanup EXIT

  # Check for help flag
  check_help "$@" && show_help

  # Display startup info
  echo "========================================"
  echo "LED Strip Emulator"
  echo "Implementation: $IMPL_NAME"
  echo "========================================"
  echo ""
  echo "Project root: $PROJECT_ROOT"
  echo "Server dir:   $IMPL_DIR"
  echo "Emulator dir: $EMU_DIR"
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
    exit 1
  fi

  # Configure strips to point to emulator
  configure_strips

  # Set RainbowColor mode
  set_rainbow_mode

  echo ""
  echo "========================================"
  echo "Use the web UI at http://localhost:$SERVER_PORT to control LEDs"
  echo "Close the emulator window to exit"
  echo "========================================"
  echo ""

  # Start the Rust emulator (this blocks until user closes it)
  cd "$EMU_DIR"
  cargo run -- --port "$EMU_PORT"

  echo ""
  echo "Emulator closed."
}

# Run main
main "$@"