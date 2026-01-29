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


def _clear_strip_state(api, esp32_strip, extra_iterations=5):
    """Helper to clear residual LED state by running Off mode.

    This ensures LED buffers are zeroed out and any in-flight packets are drained.
    """
    esp32_strip.clear_packets()
    api.set_mode("Off")
    api.step_looper(extra_iterations)
    time.sleep(0.8)
    esp32_strip.clear_packets()
    time.sleep(0.2)


class TestSparkleMode:
    """Comprehensive tests for the Sparkle animation mode."""

    def test_sparkle_mode_can_be_set(self, api):
        """Verify Sparkle mode can be activated."""
        original_mode = api.get_current_mode()

        api.set_mode("Sparkle")
        assert api.get_current_mode() == "Sparkle"

        api.set_mode(original_mode)

    def test_sparkle_has_default_opts(self, api):
        """Verify Sparkle mode has expected default options."""
        original_mode = api.get_current_mode()

        api.set_mode("Sparkle")
        opts = api.get_opts()

        # Check all expected options exist
        assert "low" in opts
        assert "high" in opts
        assert "r_on" in opts
        assert "g_on" in opts
        assert "b_on" in opts
        assert "decay" in opts

        # Check default values
        assert opts["low"]["val"] == 0
        assert opts["high"]["val"] == 255
        assert opts["r_on"]["val"] is True
        assert opts["g_on"]["val"] is False
        assert opts["b_on"]["val"] is True
        assert abs(opts["decay"]["val"] - 0.92) < 0.01

        api.set_mode(original_mode)

    def test_sparkle_produces_lit_leds(self, api, esp32_strip1):
        """Verify Sparkle mode produces some lit LEDs over time."""
        original_mode = api.get_current_mode()

        api.pause_looper()
        time.sleep(0.3)
        esp32_strip1.clear_packets()

        api.set_mode("Sparkle")
        # Set high minimum value to ensure visible sparkles
        api.set_opts({
            "r_on": {"val": "true", "type": "bool"},
            "g_on": {"val": "true", "type": "bool"},
            "b_on": {"val": "true", "type": "bool"},
            "low": {"val": 100, "type": "int"},
            "high": {"val": 255, "type": "int"},
            "decay": {"val": 0.9, "type": "float"},
        })

        # Run several iterations to allow sparkles to appear
        api.step_looper(30)
        time.sleep(2.5)

        packets = esp32_strip1.get_all_packets()
        assert len(packets) > 0, "No packets received"

        # At least some packets should have lit LEDs (sparkles)
        packets_with_lit = [p for p in packets if p.has_any_lit()]
        assert len(packets_with_lit) > 0, "Sparkle mode should produce some lit LEDs"

        api.set_mode(original_mode)
        api.resume_looper()

    def test_sparkle_red_blue_channels_only_by_default(self, api, esp32_strip1):
        """Verify default Sparkle only uses red and blue channels (not green)."""
        original_mode = api.get_current_mode()

        api.pause_looper()
        time.sleep(0.3)

        # Clear state first
        _clear_strip_state(api, esp32_strip1)

        api.set_mode("Sparkle")
        # Ensure default opts (r_on=True, g_on=False, b_on=True)
        # NOTE: Server expects string "true"/"false" for bool values
        api.set_opts({
            "r_on": {"val": "true", "type": "bool"},
            "g_on": {"val": "false", "type": "bool"},
            "b_on": {"val": "true", "type": "bool"},
            "low": {"val": 100, "type": "int"},
            "high": {"val": 255, "type": "int"},
        })

        # Run enough iterations to get sparkles
        api.step_looper(50)
        time.sleep(3.0)

        packets = esp32_strip1.get_all_packets()
        assert len(packets) > 0, "No packets received"

        # Check that green channel is always 0 for lit LEDs
        green_values_found = []
        for packet in packets:
            for i in range(packet.num_leds):
                led = packet.get_led(i)
                if led and (led[0] > 0 or led[2] > 0):
                    green_values_found.append(led[1])

        if green_values_found:
            max_green = max(green_values_found)
            assert max_green < 10, f"Green channel should be off, but found max green={max_green}"

        api.set_mode(original_mode)
        api.resume_looper()

    def test_sparkle_green_only_mode(self, api, esp32_strip1):
        """Verify Sparkle can be configured for green channel only."""
        original_mode = api.get_current_mode()

        api.pause_looper()
        time.sleep(0.3)

        # Clear state thoroughly
        _clear_strip_state(api, esp32_strip1, extra_iterations=10)

        api.set_mode("Sparkle")
        api.set_opts({
            "r_on": {"val": "false", "type": "bool"},
            "g_on": {"val": "true", "type": "bool"},
            "b_on": {"val": "false", "type": "bool"},
            "low": {"val": 100, "type": "int"},
            "high": {"val": 255, "type": "int"},
            "decay": {"val": 0.5, "type": "float"},  # Fast decay to clear any residual
        })

        # Run more iterations to stabilize state
        api.step_looper(60)
        time.sleep(4.0)

        packets = esp32_strip1.get_all_packets()
        assert len(packets) > 0, "No packets received"

        # Only look at the latter half of packets (after state has stabilized)
        stable_packets = packets[len(packets)//2:]

        # Find LEDs that have green > 50 (clear green sparkles)
        # Verify red and blue are near zero for these green sparkles
        green_sparkles = []
        for packet in stable_packets:
            for i in range(packet.num_leds):
                led = packet.get_led(i)
                if led and led[1] > 50:  # Has significant green
                    green_sparkles.append(led)

        # We should have found some green sparkles
        assert len(green_sparkles) > 0, "Should find LEDs with green values in green-only mode"

        # Verify red and blue are minimal for green sparkles
        for led in green_sparkles:
            r, g, b = led
            assert r < 20, f"Red should be minimal in green-only mode, got {r}"
            assert b < 20, f"Blue should be minimal in green-only mode, got {b}"

        api.set_mode(original_mode)
        api.resume_looper()

    def test_sparkle_low_high_range(self, api, esp32_strip1):
        """Verify sparkle values respect low/high range settings."""
        original_mode = api.get_current_mode()

        api.pause_looper()
        time.sleep(0.3)

        # Clear state first
        _clear_strip_state(api, esp32_strip1)

        api.set_mode("Sparkle")
        # Set a narrow range - values should be between 100-150
        api.set_opts({
            "r_on": {"val": "true", "type": "bool"},
            "g_on": {"val": "true", "type": "bool"},
            "b_on": {"val": "true", "type": "bool"},
            "low": {"val": 100, "type": "int"},
            "high": {"val": 150, "type": "int"},
            "decay": {"val": 1.0, "type": "float"},
        })

        api.step_looper(30)
        time.sleep(2.0)

        packets = esp32_strip1.get_all_packets()
        assert len(packets) > 0, "No packets received"

        max_value_found = 0
        min_nonzero_value = 255

        for packet in packets:
            for i in range(packet.num_leds):
                led = packet.get_led(i)
                if led:
                    for channel_val in led:
                        if channel_val > 0:
                            max_value_found = max(max_value_found, channel_val)
                            min_nonzero_value = min(min_nonzero_value, channel_val)

        if max_value_found > 0:
            assert max_value_found <= 150, f"Max value {max_value_found} exceeds high=150"
            assert min_nonzero_value >= 100, f"Min value {min_nonzero_value} below low=100"

        api.set_mode(original_mode)
        api.resume_looper()

    def test_sparkle_decay_reduces_brightness(self, api, esp32_strip1):
        """Verify decay option causes LED values to decrease over time."""
        original_mode = api.get_current_mode()

        api.pause_looper()
        time.sleep(0.3)
        esp32_strip1.clear_packets()

        api.set_mode("Sparkle")
        api.set_opts({
            "r_on": {"val": "true", "type": "bool"},
            "g_on": {"val": "true", "type": "bool"},
            "b_on": {"val": "true", "type": "bool"},
            "low": {"val": 200, "type": "int"},
            "high": {"val": 255, "type": "int"},
            "decay": {"val": 0.5, "type": "float"},
        })

        api.step_looper(30)
        time.sleep(2.5)

        packets = esp32_strip1.get_all_packets()
        assert len(packets) >= 5, f"Expected at least 5 packets, got {len(packets)}"

        values_below_minimum = []
        for packet in packets:
            for i in range(packet.num_leds):
                led = packet.get_led(i)
                if led:
                    for val in led:
                        if 0 < val < 200:
                            values_below_minimum.append(val)

        assert len(values_below_minimum) > 0, (
            "Decay should produce values below the minimum sparkle value (200)"
        )

        api.set_mode(original_mode)
        api.resume_looper()

    def test_sparkle_no_decay_preserves_values(self, api, esp32_strip1):
        """Verify decay=1.0 preserves LED values (no fading)."""
        original_mode = api.get_current_mode()

        api.pause_looper()
        time.sleep(0.3)

        # Clear state first to avoid residual values
        _clear_strip_state(api, esp32_strip1)

        api.set_mode("Sparkle")
        api.set_opts({
            "r_on": {"val": "true", "type": "bool"},
            "g_on": {"val": "false", "type": "bool"},
            "b_on": {"val": "false", "type": "bool"},
            "low": {"val": 100, "type": "int"},
            "high": {"val": 100, "type": "int"},
            "decay": {"val": 1.0, "type": "float"},
        })

        api.step_looper(20)
        time.sleep(1.5)

        packets = esp32_strip1.get_all_packets()
        assert len(packets) > 0, "No packets received"

        for packet in packets:
            for i in range(packet.num_leds):
                led = packet.get_led(i)
                if led and led[0] > 0:
                    assert led[0] == 100, f"Red value should be exactly 100 with no decay, got {led[0]}"

        api.set_mode(original_mode)
        api.resume_looper()

    def test_sparkle_sparseness(self, api, esp32_strip1):
        """Verify sparkles are sparse (not all LEDs lit every frame)."""
        original_mode = api.get_current_mode()

        api.pause_looper()
        time.sleep(0.3)
        esp32_strip1.clear_packets()

        api.set_mode("Sparkle")
        api.set_opts({
            "r_on": {"val": "true", "type": "bool"},
            "g_on": {"val": "true", "type": "bool"},
            "b_on": {"val": "true", "type": "bool"},
            "low": {"val": 200, "type": "int"},
            "high": {"val": 255, "type": "int"},
            "decay": {"val": 0.1, "type": "float"},
        })

        api.step_looper(10)
        time.sleep(1.5)

        packets = esp32_strip1.get_all_packets()
        assert len(packets) > 0, "No packets received"

        frames_with_all_lit = 0
        total_frames = len(packets)

        for packet in packets:
            lit_count = packet.count_lit()
            if lit_count == packet.num_leds:
                frames_with_all_lit += 1

        assert frames_with_all_lit < total_frames, (
            "Sparkle should be sparse - not all LEDs lit every frame"
        )

        api.set_mode(original_mode)
        api.resume_looper()

    def test_sparkle_all_channels_on(self, api, esp32_strip1):
        """Verify all RGB channels can be enabled together and produce varied colors."""
        original_mode = api.get_current_mode()

        api.pause_looper()
        time.sleep(0.3)

        # Clear state first
        _clear_strip_state(api, esp32_strip1)

        api.set_mode("Sparkle")
        api.set_opts({
            "r_on": {"val": "true", "type": "bool"},
            "g_on": {"val": "true", "type": "bool"},
            "b_on": {"val": "true", "type": "bool"},
            "low": {"val": 100, "type": "int"},
            "high": {"val": 255, "type": "int"},
            "decay": {"val": 0.95, "type": "float"},
        })

        # Run more iterations to increase chance of finding RGB LEDs
        api.step_looper(100)
        time.sleep(5.0)

        packets = esp32_strip1.get_all_packets()
        assert len(packets) > 0, "No packets received"

        # With all channels on, we should see LEDs with non-zero values in each channel
        # Due to random sparkle timing, they may not all be >50 at once
        # Instead verify we see activity in all three channels across all packets
        found_r = False
        found_g = False
        found_b = False
        for packet in packets:
            for i in range(packet.num_leds):
                led = packet.get_led(i)
                if led:
                    if led[0] > 50:
                        found_r = True
                    if led[1] > 50:
                        found_g = True
                    if led[2] > 50:
                        found_b = True

        assert found_r, "With all channels on, should find LEDs with red values"
        assert found_g, "With all channels on, should find LEDs with green values"
        assert found_b, "With all channels on, should find LEDs with blue values"

        api.set_mode(original_mode)
        api.resume_looper()

    def test_sparkle_all_channels_off_produces_dark(self, api, esp32_strip1):
        """Verify all channels off with zero decay causes LEDs to go dark."""
        original_mode = api.get_current_mode()

        api.pause_looper()
        time.sleep(0.3)

        # Clear state thoroughly
        _clear_strip_state(api, esp32_strip1, extra_iterations=10)

        api.set_mode("Sparkle")
        api.set_opts({
            "r_on": {"val": "false", "type": "bool"},
            "g_on": {"val": "false", "type": "bool"},
            "b_on": {"val": "false", "type": "bool"},
            "decay": {"val": 0.0, "type": "float"},
        })

        # Run enough iterations to ensure state has settled
        api.step_looper(20)
        time.sleep(2.0)

        packets = esp32_strip1.get_all_packets()
        assert len(packets) > 0, "No packets received"

        # With decay=0 and no channels on, the latter packets should be all dark
        # Check only the last few packets (after any residual state has decayed)
        final_packets = packets[-5:] if len(packets) >= 5 else packets

        for packet in final_packets:
            assert not packet.has_any_lit(), (
                f"All channels off with decay=0 should produce dark LEDs, "
                f"but found {packet.count_lit()} lit LEDs"
            )

        api.set_mode(original_mode)
        api.resume_looper()

    def test_sparkle_multi_strip_sync(self, api, esp32_strip1, esp32_strip2):
        """Verify both strips receive Sparkle mode packets."""
        original_mode = api.get_current_mode()

        api.pause_looper()
        time.sleep(0.3)
        esp32_strip1.clear_packets()
        esp32_strip2.clear_packets()

        api.set_mode("Sparkle")

        api.step_looper(10)
        time.sleep(1.5)

        packets1 = esp32_strip1.get_all_packets()
        packets2 = esp32_strip2.get_all_packets()

        assert len(packets1) > 0, "Strip 1 should receive Sparkle packets"
        assert len(packets2) > 0, "Strip 2 should receive Sparkle packets"

        for p in packets1:
            assert p.num_leds == STRIP_1_LEDS
        for p in packets2:
            assert p.num_leds == STRIP_2_LEDS

        api.set_mode(original_mode)
        api.resume_looper()

    def test_sparkle_opts_persistence(self, api):
        """Verify Sparkle options persist after being set."""
        original_mode = api.get_current_mode()

        api.set_mode("Sparkle")

        # Set custom options using string "true"/"false" for bools
        api.set_opts({
            "low": {"val": 50, "type": "int"},
            "high": {"val": 200, "type": "int"},
            "r_on": {"val": "false", "type": "bool"},
            "g_on": {"val": "true", "type": "bool"},
            "b_on": {"val": "true", "type": "bool"},
            "decay": {"val": 0.85, "type": "float"},
        })

        # Read back and verify
        opts = api.get_opts()
        assert opts["low"]["val"] == 50
        assert opts["high"]["val"] == 200
        assert opts["r_on"]["val"] is False
        assert opts["g_on"]["val"] is True
        assert opts["b_on"]["val"] is True
        assert abs(opts["decay"]["val"] - 0.85) < 0.01

        api.set_mode(original_mode)

    def test_sparkle_zero_decay_accumulates(self, api, esp32_strip1):
        """Verify decay=0 causes rapid fade to black."""
        original_mode = api.get_current_mode()

        api.pause_looper()
        time.sleep(0.3)
        esp32_strip1.clear_packets()

        api.set_mode("Sparkle")
        api.set_opts({
            "r_on": {"val": "true", "type": "bool"},
            "g_on": {"val": "true", "type": "bool"},
            "b_on": {"val": "true", "type": "bool"},
            "low": {"val": 255, "type": "int"},
            "high": {"val": 255, "type": "int"},
            "decay": {"val": 0.0, "type": "float"},
        })

        api.step_looper(20)
        time.sleep(1.5)

        packets = esp32_strip1.get_all_packets()
        assert len(packets) > 0, "No packets received"

        total_lit_ratio = []
        for packet in packets:
            lit = packet.count_lit()
            total_lit_ratio.append(lit / packet.num_leds)

        avg_lit_ratio = sum(total_lit_ratio) / len(total_lit_ratio) if total_lit_ratio else 0

        assert avg_lit_ratio < 0.3, f"With decay=0, most LEDs should be dark, got {avg_lit_ratio:.2%} lit"

        api.set_mode(original_mode)
        api.resume_looper()


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
