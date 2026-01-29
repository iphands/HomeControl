#!/bin/bash
#
# Start the LED controller server in production mode
# For use with real ESP32 LED controllers
#

set -e

# Source common functions
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

# Help text
show_help() {
  show_usage "$0" "<py|python|rs|rust>" \
    "Start the LED controller server in production mode." \
    ""
  exit 0
}

# Main function
main() {
  # Check for help flag
  check_help "$@" && show_help

  # Parse subcommand
  if ! parse_impl_subcommand "$1"; then
    echo ""
    show_usage "$0" "<py|python|rs|rust>" \
      "Start the LED controller server in production mode."
    exit 1
  fi

  # Check Python venv if using Python
  if [[ "$IMPL" == "python" ]]; then
    check_python_venv || exit 1
  fi

  # Kill any existing server
  kill_port_processes
  wait_for_port_release || true

  # Start server
  echo "Starting $IMPL_NAME LED controller server..."
  echo "  Project root: $PROJECT_ROOT"
  echo "  App directory: $IMPL_DIR"
  echo "  Press Ctrl+C to stop"
  echo ""

  cd "$IMPL_DIR"
  eval "$START_CMD"
}

# Run main
main "$@"
