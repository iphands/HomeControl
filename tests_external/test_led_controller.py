"""
Functional tests for the LED controller API.

These tests interact with the API via REST and verify that the correct
UDP packets are sent to fake ESP32 devices.

To run these tests:
1. Start the server in debug mode: cd app && python -m __init__ --debug
2. Run: pytest test_led_controller.py -v

Or use the run_tests.py script which handles server startup.
"""

import pytest
import time
from fake_esp32 import FakeESP32, LEDPacket
from api_client import LEDControllerAPI


# Test configuration - should match server config for testing
STRIP_1_ID = 1
STRIP_2_ID = 2
STRIP_1_LEDS = 67
STRIP_2_LEDS = 83
UDP_PORT_STRIP1 = 14210  # Test port for strip 1
UDP_PORT_STRIP2 = 14211  # Test port for strip 2
API_URL = "http://localhost:5000"


@pytest.fixture(scope="module")
def api():
    """Create API client and configure strips for testing."""
    client = LEDControllerAPI(API_URL)
    # Verify server is running in debug mode
    if not client.is_debug_mode():
        pytest.skip("Server not running in debug mode. Start with: python -m __init__ --debug")

    # Configure strips to send to localhost on test ports
    client.configure_strip(STRIP_1_ID, hostname="127.0.0.1", port=UDP_PORT_STRIP1)
    client.configure_strip(STRIP_2_ID, hostname="127.0.0.1", port=UDP_PORT_STRIP2)

    return client


@pytest.fixture
def esp32_strip1():
    """Create fake ESP32 for strip 1."""
    esp = FakeESP32(device_id=STRIP_1_ID, port=UDP_PORT_STRIP1)
    esp.start()
    time.sleep(0.3)  # Ensure socket is ready
    yield esp
    esp.stop()


@pytest.fixture
def esp32_strip2():
    """Create fake ESP32 for strip 2."""
    esp = FakeESP32(device_id=STRIP_2_ID, port=UDP_PORT_STRIP2)
    esp.start()
    time.sleep(0.3)  # Ensure socket is ready
    yield esp
    esp.stop()


class TestAPIBasics:
    """Test basic API functionality."""

    def test_get_modes(self, api):
        """Verify we can get the list of available modes."""
        modes = api.get_modes()
        assert isinstance(modes, list)
        assert len(modes) > 0
        # Check some expected modes exist
        expected = ["NightRider", "RainbowCycle", "Off", "Solid"]
        for mode in expected:
            assert mode in modes, f"Expected mode {mode} not found"

    def test_get_strips(self, api):
        """Verify we can get strip configuration."""
        strips = api.get_strips()
        assert len(strips) == 2
        assert strips[0]["id"] == STRIP_1_ID
        assert strips[1]["id"] == STRIP_2_ID

    def test_brightness_get_set(self, api):
        """Test getting and setting brightness."""
        original = api.get_brightness()

        # Set new brightness
        api.set_brightness(100)
        assert api.get_brightness() == 100

        api.set_brightness(200)
        assert api.get_brightness() == 200

        # Restore original
        api.set_brightness(original)

    def test_delay_get_set(self, api):
        """Test getting and setting delay."""
        original = api.get_delay()

        api.set_delay(0.05)
        assert abs(api.get_delay() - 0.05) < 0.001

        api.set_delay(0.1)
        assert abs(api.get_delay() - 0.1) < 0.001

        # Restore original
        api.set_delay(original)

    def test_mode_get_set(self, api):
        """Test getting and setting mode."""
        original = api.get_current_mode()

        api.set_mode("Off")
        assert api.get_current_mode() == "Off"

        api.set_mode("Solid")
        assert api.get_current_mode() == "Solid"

        # Restore original
        api.set_mode(original)


class TestLooperControl:
    """Test the debug looper control functionality."""

    def test_looper_state(self, api):
        """Test getting looper state."""
        state = api.get_looper_state()
        assert "debug_mode" in state
        assert "state" in state
        assert state["debug_mode"] is True

    def test_looper_pause_resume(self, api):
        """Test pausing and resuming the looper."""
        # Pause
        result = api.pause_looper()
        assert result["state"] == "paused"

        # Resume
        result = api.resume_looper()
        assert result["state"] == "running"

    def test_looper_step(self, api):
        """Test stepping the looper."""
        # Set fast delay for predictable timing
        original_delay = api.get_delay()
        api.set_delay(0.01)

        # Pause first
        api.pause_looper()

        # Step 3 iterations
        result = api.step_looper(3)
        assert result["iterations_remaining"] == 3

        # Wait for iterations to complete (3 * 0.01 + buffer)
        time.sleep(0.5)

        # Should be paused again
        state = api.get_looper_state()
        assert state["state"] == "paused"

        # Restore and resume for other tests
        api.set_delay(original_delay)
        api.resume_looper()


class TestUDPOutput:
    """Test that correct UDP packets are sent to fake ESP32 devices."""

    def test_off_mode_sends_zeros(self, api, esp32_strip1):
        """Verify Off mode sends all zeros."""
        original_mode = api.get_current_mode()

        # Pause looper and clear any pending packets
        api.pause_looper()
        time.sleep(0.3)
        esp32_strip1.clear_packets()

        # Set mode (which may set delay via load_cb)
        api.set_mode("Off")

        # Run one iteration and wait for completion
        api.step_looper(1)
        time.sleep(0.5)

        # Wait for packet
        packet = esp32_strip1.get_packet(timeout=2.0)

        # Verify
        assert packet is not None, "No packet received"
        assert packet.device_id == STRIP_1_ID
        assert packet.num_leds == STRIP_1_LEDS
        assert not packet.has_any_lit(), "Off mode should have all LEDs off"

        # Cleanup
        api.set_mode(original_mode)
        api.resume_looper()

    def test_brightness_in_packet(self, api, esp32_strip1):
        """Verify brightness is correctly sent in packets."""
        original_brightness = api.get_brightness()

        # Pause looper and clear any pending packets
        api.pause_looper()
        time.sleep(0.3)
        esp32_strip1.clear_packets()

        # Set specific brightness
        api.set_brightness(123)
        api.set_mode("Off")  # Simple mode that won't change brightness

        # Run one iteration and wait for completion
        api.step_looper(1)
        time.sleep(0.5)

        # Wait for packet
        packet = esp32_strip1.get_packet(timeout=2.0)

        # Verify brightness
        assert packet is not None
        assert packet.brightness == 123

        # Cleanup
        api.set_brightness(original_brightness)
        api.resume_looper()

    def test_solid_mode_color(self, api, esp32_strip1):
        """Verify Solid mode sends the correct color."""
        original_mode = api.get_current_mode()

        # Pause looper and clear any pending packets
        api.pause_looper()
        time.sleep(0.3)
        esp32_strip1.clear_packets()

        # Set mode and options
        api.set_mode("Solid")
        api.set_opts({
            "color": {"val": "#ff0000", "type": "color"}  # Red
        })

        # Run one iteration and wait for completion
        api.step_looper(1)
        time.sleep(0.5)

        # Wait for packet
        packet = esp32_strip1.get_packet(timeout=2.0)

        # Verify all LEDs are red
        assert packet is not None
        for i in range(min(10, packet.num_leds)):  # Check first 10
            led = packet.get_led(i)
            assert led is not None
            assert led[0] == 255, f"LED {i} red should be 255"
            assert led[1] == 0, f"LED {i} green should be 0"
            assert led[2] == 0, f"LED {i} blue should be 0"

        # Cleanup
        api.set_mode(original_mode)
        api.resume_looper()

    def test_sequence_increments(self, api, esp32_strip1):
        """Verify sequence number increments between packets."""
        # Pause looper and clear packets
        api.pause_looper()
        time.sleep(0.3)
        esp32_strip1.clear_packets()

        api.set_mode("Off")

        # Run multiple iterations
        api.step_looper(5)
        time.sleep(1.5)

        # Get packets
        packets = esp32_strip1.get_all_packets()
        assert len(packets) >= 3, f"Expected at least 3 packets, got {len(packets)}"

        # Verify sequence increments (with rollover handling)
        for i in range(1, len(packets)):
            prev_seq = packets[i - 1].sequence
            curr_seq = packets[i].sequence
            expected = (prev_seq + 1) % 256
            assert curr_seq == expected, (
                f"Sequence should increment: {prev_seq} -> {curr_seq}, expected {expected}"
            )

        api.resume_looper()

    def test_nightrider_has_lit_leds(self, api, esp32_strip1):
        """Verify NightRider mode lights up some LEDs."""
        original_mode = api.get_current_mode()

        # Pause looper and clear packets
        api.pause_looper()
        time.sleep(0.3)
        esp32_strip1.clear_packets()

        api.set_mode("NightRider")

        # Run several iterations
        api.step_looper(10)
        time.sleep(1.5)

        # Get packets
        packets = esp32_strip1.get_all_packets()
        assert len(packets) > 0, "No packets received"

        # At least one packet should have lit LEDs
        any_lit = any(p.has_any_lit() for p in packets)
        assert any_lit, "NightRider should have some LEDs lit"

        # Cleanup
        api.set_mode(original_mode)
        api.resume_looper()


class TestMultipleStrips:
    """Test that multiple strips receive correct packets."""

    def test_both_strips_receive_packets(self, api, esp32_strip1, esp32_strip2):
        """Verify both strips receive packets on each iteration."""
        # Pause looper and clear packets
        api.pause_looper()
        time.sleep(0.3)
        esp32_strip1.clear_packets()
        esp32_strip2.clear_packets()

        api.set_mode("Off")

        # Run iterations
        api.step_looper(3)
        time.sleep(1.5)

        # Both should have received packets
        packets1 = esp32_strip1.get_all_packets()
        packets2 = esp32_strip2.get_all_packets()

        assert len(packets1) >= 2, f"Strip 1 should receive packets, got {len(packets1)}"
        assert len(packets2) >= 2, f"Strip 2 should receive packets, got {len(packets2)}"

        api.resume_looper()

    def test_strips_have_correct_led_count(self, api, esp32_strip1, esp32_strip2):
        """Verify each strip receives packets with correct LED count."""
        # Pause looper and clear packets
        api.pause_looper()
        time.sleep(0.3)
        esp32_strip1.clear_packets()
        esp32_strip2.clear_packets()

        api.set_mode("Off")

        api.step_looper(1)
        time.sleep(0.5)

        packet1 = esp32_strip1.get_packet(timeout=1.0)
        packet2 = esp32_strip2.get_packet(timeout=1.0)

        if packet1:
            assert packet1.num_leds == STRIP_1_LEDS, f"Strip 1 should have {STRIP_1_LEDS} LEDs"
        if packet2:
            assert packet2.num_leds == STRIP_2_LEDS, f"Strip 2 should have {STRIP_2_LEDS} LEDs"

        api.resume_looper()


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
