# HomeCtrl - LED Controller System

## Project Overview

This is a networked LED strip animation system with three main components:

1. **Server/API** - A Python Flask application that manages LED animations and broadcasts state to devices
2. **Embedded Firmware** - ESP32/ESP8266 code that receives UDP packets and controls physical LED strips
3. **External Test Suite** - A standalone test program that validates the system without sharing code with the server

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
| `/modes` | List available animation modes |
| `/modes/current` | Get/set current mode |
| `/brightness` | Get/set LED brightness |
| `/delay` | Get/set animation speed |
| `/opts` | Get/set mode-specific options (colors, etc.) |
| `/strips` | Get strip configuration |
| `/looper` | Control animation loop (debug mode only) |

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
