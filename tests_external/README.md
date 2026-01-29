# LED Controller Functional Tests

This is an independent test suite for the LED controller API. It's designed to:

1. **Test via REST API** - All tests interact with the server via HTTP
2. **Simulate ESP32 devices** - Fake UDP listeners capture LED packets
3. **Be server-agnostic** - Can test any server implementation (Python, Rust, etc.)

## Setup

```bash
cd tests_external
python -m venv venv
source venv/bin/activate
pip install -r requirements.txt
```

## Running Tests

### Option 1: Manual Server Start

Start the server in debug mode first:

```bash
cd ../app
source ../venv/bin/activate
python -m __init__ --debug
```

Then run tests:

```bash
cd tests_external
source venv/bin/activate
pytest test_led_controller.py -v
```

### Option 2: Automatic Server Start

The test runner can start the server for you:

```bash
python run_tests.py --start-server -v
```

### Running Specific Tests

```bash
pytest test_led_controller.py -v -k test_brightness
pytest test_led_controller.py -v -k TestUDPOutput
```

## Debug Mode

The server must be running with `--debug` flag for tests to work. This enables:

- **Loop control**: Pause/resume the animation loop
- **Step mode**: Run specific number of iterations
- **Deterministic testing**: Control exactly when packets are sent

Without debug mode, the `/looper` endpoint returns an error.

## Test Structure

- `fake_esp32.py` - Simulates ESP32 UDP listener
- `api_client.py` - REST API client for the LED controller
- `test_led_controller.py` - Pytest test cases

## Writing New Tests

```python
def test_example(api, esp32_strip1):
    # Pause the loop for deterministic testing
    api.pause_looper()
    esp32_strip1.clear_packets()

    # Set up state via API
    api.set_mode("Solid")
    api.set_brightness(100)

    # Run exactly 1 iteration
    api.step_looper(1)

    # Check what was sent to the ESP32
    packet = esp32_strip1.get_packet(timeout=2.0)
    assert packet is not None
    assert packet.brightness == 100

    # Cleanup
    api.resume_looper()
```

## Protocol Reference

UDP packet format (bytes):

| Offset | Size | Description |
|--------|------|-------------|
| 0 | 1 | Message type (1 = LED_STRIP) |
| 1 | 1 | Sequence number (0-255) |
| 2 | 1 | Brightness (0-255) |
| 3 | 1 | Number of LEDs |
| 4 | 1 | Device ID |
| 5+ | 3*N | RGB data (3 bytes per LED) |
