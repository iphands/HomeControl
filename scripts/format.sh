#!/bin/bash
#
# Format all code (Rust and Python)
#

set -e

# Source common functions
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

# Help text
show_help() {
  echo "Format all code (Rust and Python)"
  echo ""
  echo "Usage: $(basename "$0") [--help]"
  echo ""
  echo "Options:"
  echo "  --help    Show this help message"
  exit 0
}

# Format Rust code
format_rust() {
  echo "Formatting Rust code..."
  pushd "$APP_RS_DIR" > /dev/null
  rustfmt $(find src -type f -name '*.rs')
  popd > /dev/null
  echo "Rust formatting complete"
}

# Format Python code
format_python() {
  echo "Formatting Python code..."
  check_python_venv || exit 1

  pushd "$APP_DIR" > /dev/null
  source "$APP_VENV/bin/activate"
  black .
  popd > /dev/null
  echo "Python formatting complete"
}

# Main function
main() {
  # Check for help flag
  check_help "$@" && show_help

  echo "========================================"
  echo "Formatting Code"
  echo "========================================"
  echo ""

  format_rust
  echo ""
  format_python

  echo ""
  echo "========================================"
  echo "Formatting Complete"
  echo "========================================"
}

# Run main
main "$@"
