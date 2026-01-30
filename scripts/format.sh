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

format_rust_app_rs() {
  echo "Formatting Rust code (app_rs)..."
  pushd "$APP_RS_DIR" > /dev/null
  cargo fmt
  popd > /dev/null
  echo "Rust formatting complete (app_rs)"
}

format_rust_emu() {
  echo "Formatting Rust code (emu)..."
  pushd "$PROJECT_ROOT/emu" > /dev/null
  cargo fmt
  popd > /dev/null
  echo "Rust formatting complete (emu)"
}

# Format Python code
format_python_api() {
  echo "Formatting Python code..."
  check_python_venv || exit 1

  pushd "$APP_DIR" > /dev/null
  source "$APP_VENV/bin/activate"
  black .
  popd > /dev/null
  echo "Python formatting complete"
}

main() {
  check_help "$@" && show_help
  format_rust_app_rs
  format_rust_emu
  format_python_api
}

# Run main
main "$@"
