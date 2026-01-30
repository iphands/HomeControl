# HomeCtrl - LED Controller System

## Overview

Networked LED strip animation system with Python and Rust server implementations, ESP32 firmware, and isolated external tests.

**Key principle**: Both server implementations must pass the same test suite to prove behavioral parity.

## Architecture

Three components:
1. **Server** - Python/Flask or Rust/Actix-web REST API + animation loop + UDP broadcasts
2. **Embedded Firmware** - ESP32/ESP8266 receives UDP, drives WS2811/WS2812 strips
3. **External Tests** - Standalone validation via REST API + UDP capture (no shared code with server)

## UDP Protocol

5-byte header + RGB data:
- Byte 0: Message type (1=LED_STRIP)
- Byte 1: Sequence (0-255)
- Byte 2: Brightness (0-255)
- Byte 3: LED count
- Byte 4: Device ID
- Bytes 5+: 3 bytes per LED (RGB)

Multi-strip support via device ID filtering.

## API Endpoints

| Endpoint | Purpose |
|----------|---------|
| `/api/modes` | List/set animation modes |
| `/api/brightness` | Get/set brightness |
| `/api/delay` | Get/set animation speed |
| `/api/opts` | Mode-specific options |
| `/api/strips` | Strip configuration |
| `/api/looper` | Loop control (debug mode only) |

## Debug Mode

`--debug` flag enables deterministic testing:
- Pause/resume animation loop
- Step exact iterations
- Reconfigure strip addresses

## Commands

### Docs
In the app_rs/ Rust project use `cargo docs --no-deps`
Be sure and fix the warnings.
Then you can look at the docs to quickly learn about the project ABI.
The docs will be in app_rs/target/doc/homectrl (If you read this in bulk compact the data when taking in context its too much keep it all around)


### Formatting
Use:  ./scripts/format.sh

### Tests
```bash
./scripts/run-tests.sh [python|rust|both]
cd tests_external && pytest test_led_controller.py -v -k <test_name>
```

## Running Python
You must run Python after sourcing venv!
There are two venv set up
- venv/ in the repo root used for Python impl of the API (ie running app/...)
- tests_external/venv/ used for running any Python in tests_external/...

## Container notes
Check if you are running on a container! A file: /home/iphands/IM_A_CONTAINER will exist if you are.
Otherwise you are not

### Managing venv in container
If you are DONT try to manage venv (NO pip install) I will do that if you are running in a container.
You can try to source the venv file
You can edit the requirements.txt
But I will need to be the one running pip install's

### Running GUI things in the container
In this env you wont be able to launch/run GUI apps.
Please tell me to do so and I can run it for you (and share output).

## Code Style

### Rust
- Max width: 128, 4 spaces, Unix newlines
- snake_case functions, PascalCase types
- `Result<T, E>` with `?` operator
- Import order: std → crates → local

### Python
- Black formatter (default 88 chars)
- Group/sort imports (stdlib → third-party → local)
- snake_case functions, PascalCase classes
- Use click library for all arg parsing

### Git
- Prefix: `[rs]` Rust, `[py]` Python, `[test]` tests
- No Co-Authored-By or AI mentions in commits

## Project Structure

- `app/` - Python server
- `app_rs/` - Rust server
- `tests_external/` - Standalone test suite
- `scripts/` - Automation
- `frontend/` - Web UI
- `embedded/` - ESP32 firmware

