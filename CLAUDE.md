# HomeCtrl - LED Controller System

## Project Overview

This is a networked LED strip animation system with three main components:

1. **Server/API** - A Python Flask application that manages LED animations and broadcasts state to devices
2. **Embedded Firmware** - ESP32/ESP8266 code that receives UDP packets and controls physical LED strips
3. **External Test Suite** - A standalone test program that validates the system without sharing code with the server

NOTE: There is now a Rust based clone of the server / api it exists at app_rs/
The Rust and Python implementations should behave the same, and we can prove that by running the external test suite.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│              Server (Python/Flask)                  │
│  - REST API for configuration                       │
│  - Animation loop (threaded)                        │
│  - UDP packet generation                            │
│  - Web UI for user control                          │
└─────────────────────────────────────────────────────┘
                      │
                      │ UDP packets (port 4210)
                      ▼
┌─────────────────────────────────────────────────────┐
│           ESP32 Devices (multiple)                  │
│  - WiFi UDP listener                                │
│  - FastLED WS2811/WS2812 driver                     │
│  - Device ID filtering                              │
└─────────────────────────────────────────────────────┘
```

## Goals and Philosophy

### Multi-Strip Support
The system controls multiple LED strips independently. Each strip has:
- A unique device ID
- Its own LED count
- A network address (hostname + port)

All strips share the same animation mode and settings, but packets are addressed to specific devices.

### Server-Agnostic Testing
The test suite is intentionally isolated from the server code:
- Tests interact only via REST API (HTTP)
- Tests consume output via UDP (simulating ESP32 devices)
- No shared imports or runtime with the server
- This enables rewriting the server in another language (e.g., Rust) while keeping the same test suite

### Debug Mode
The server supports a debug mode (`--debug` flag) that enables:
- Pausing the animation loop
- Stepping through exact numbers of iterations
- Reconfiguring strip network addresses (for test redirection)

This allows deterministic testing without timing issues.

## UDP Protocol

The server broadcasts LED state via UDP packets:

```
Byte 0:    Message type (1 = LED strip)
Byte 1:    Sequence number (0-255, increments per packet)
Byte 2:    Brightness (0-255)
Byte 3:    Number of LEDs
Byte 4:    Device ID
Bytes 5+:  RGB data (3 bytes per LED)
```

Key properties:
- Stateless - each packet contains complete strip state
- Sequence numbers prevent duplicate/out-of-order processing
- Device ID enables multi-device addressing on same network

## API Endpoints

| Endpoint | Purpose |
|----------|---------|
| `/api/modes` | List available animation modes |
| `/api/modes/current` | Get/set current mode |
| `/api/brightness` | Get/set LED brightness |
| `/api/delay` | Get/set animation speed |
| `/api/opts` | Get/set mode-specific options (colors, etc.) |
| `/api/strips` | Get strip configuration |
| `/api/looper` | Control animation loop (debug mode only) |

## Animation Modes

The system includes multiple animation modes:
- Moving patterns (NightRider, Collider)
- Color cycling (RainbowCycle, Christmas, MardiGras)
- Effects (Sparkle, Breathe)
- Static (Solid, White, Off)

Each mode has configurable options (colors, speeds, behaviors).

## Running the System

### Production
```bash
./scripts/start-server.sh
```
Starts the server for real ESP32 devices.

### Testing
```bash
./scripts/run-tests.sh
```
Starts server in debug mode, runs test suite, cleans up.

## Test Suite Design

The test suite validates:
- API endpoints work correctly
- UDP packets contain correct data
- Animation modes produce expected output
- Multi-strip coordination works
- Loop control (pause/step/resume) functions properly

Tests use "fake ESP32" listeners that capture UDP packets for inspection.

## Future Direction

The server may be rewritten in Rust for performance. The test suite is designed to remain unchanged - it tests the external behavior (REST API + UDP output) rather than internal implementation.

When rewriting:
- Maintain the same API endpoints and response formats
- Maintain the same UDP packet structure
- The test suite should pass without modification

## Git Commit Guidelines

- Do NOT add Co-Authored-By lines mentioning Claude or AI
- Do NOT mention Claude or AI assistance in commit messages
- Use the existing commit message style with prefixes like `[rs]`, `[test]`, `[feature]`, etc.

## Build/Test/Lint Commands

### Rust (app_rs/)
```bash
# Build
cd app_rs && cargo build
cd app_rs && cargo build --release

# Run
cd app_rs && cargo run
cd app_rs && cargo run -- --debug

# Check/Lint
cd app_rs && cargo check
cd app_rs && cargo clippy

# Format
cd app_rs && rustfmt $(find src -name '*.rs')
# or use: ./scripts/format.sh
```

### Python (app/)
```bash
# Setup virtualenv
python -m venv venv && source venv/bin/activate && pip install -r requirements.txt

# Run (from repo root)
source venv/bin/activate && cd app && python -m __init__
source venv/bin/activate && cd app && python -m __init__ --debug

# Format
cd app && black .
# or use: ./scripts/format.sh
```

### External Tests (tests_external/)
```bash
# Run all tests against both implementations
./scripts/run-tests.sh both
./scripts/run-tests.sh python
./scripts/run-tests.sh rust

# Run single test manually (server must be in debug mode)
cd tests_external && source venv/bin/activate
pytest test_led_controller.py -v -k test_brightness
pytest test_led_controller.py -v -k TestUDPOutput
pytest test_led_controller.py::test_specific_function -v
```

## Code Style Guidelines

### Rust
- Max width: 128 columns (rustfmt.toml)
- 4 spaces, no hard tabs
- Unix newlines
- Reorder imports and modules
- Use snake_case for functions/variables
- Use PascalCase for types/structs/enums
- Error handling: Use `Result<T, E>` with `?` operator
- Organize imports: std first, then crates, then local modules

### Python
- Formatter: Black (no specific line length set, use default 88)
- Imports: Group stdlib, third-party, local; sort alphabetically
- Naming: snake_case for functions/variables, PascalCase for classes
- Error handling: Use exceptions, catch specific types
- Type hints: Optional but encouraged for function signatures
- No trailing whitespace, files end with newline

### General
- Prefix commits with implementation: `[rs]` for Rust, `[py]` for Python, `[test]` for tests
- Do NOT add Co-Authored-By lines mentioning Claude/AI
- Do NOT mention AI assistance in commit messages
- Keep implementations behaviorally identical (test both with `run-tests.sh both`)

## Project Structure

- `app/` - Python Flask server implementation
- `app_rs/` - Rust Actix-web server implementation
- `tests_external/` - Standalone test suite (no shared imports with server)
- `scripts/` - Build and test automation
- `frontend/` - Static web UI
- `embedded/` - ESP32/ESP8266 firmware

## Testing Notes

- Tests require server in `--debug` mode for deterministic behavior
- Tests interact via REST API only (no direct code sharing)
- Tests simulate ESP32 devices via UDP listeners
- UDP protocol: Byte 0=msg_type, 1=sequence, 2=brightness, 3=num_leds, 4=device_id, 5+=RGB data

## Dependencies

- Rust: See `app_rs/Cargo.toml` (actix-web, serde, tokio)
- Python app: See `requirements.txt` (Flask, black)
- Python tests: See `tests_external/requirements.txt` (pytest, requests)
