#!/bin/bash
#
# Start the LED controller server in production mode (no debug)
# For use with real ESP32 LED controllers
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
APP_DIR="$PROJECT_ROOT/app"
VENV_DIR="$PROJECT_ROOT/venv"

# Check venv exists
if [[ ! -d "$VENV_DIR" ]]; then
    echo "Error: Virtual environment not found at $VENV_DIR"
    echo "Create it with: python -m venv $VENV_DIR && source $VENV_DIR/bin/activate && pip install -r requirements.txt"
    exit 1
fi

# Activate venv and start server
cd "$APP_DIR"
source "$VENV_DIR/bin/activate"

echo "Starting LED controller server..."
echo "  Project root: $PROJECT_ROOT"
echo "  App directory: $APP_DIR"
echo "  Press Ctrl+C to stop"
echo ""

python -m __init__
