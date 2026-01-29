import _thread
import copy
import threading
import time
from dataclasses import dataclass, field
from typing import List, Dict, Any, Optional, Callable
from strip_ctl import Strip

DEFAULT_DELAY = 0.0250


@dataclass
class StripState:
    """Per-strip state including mode, delay, and timing."""
    strip: Strip
    mode: Any  # Mode instance
    delay: float = DEFAULT_DELAY
    last_update: float = field(default_factory=time.time)

    def get_brightness(self) -> int:
        return self.strip.get_brightness()

    def set_brightness(self, val: int):
        self.strip.set_brightness(val)

    def get_mode_name(self) -> str:
        return self.mode.name

    def get_opts(self) -> Dict[str, Any]:
        return self.mode.get_opts()

    def set_opts(self, opts: Dict[str, Any]):
        self.mode.set_opts(copy.deepcopy(opts))


# Initialize strips and per-strip states
strips = [
    Strip(1, "esp32c6-00.lan", 67),
    Strip(2, "esp32c6-01.lan", 83),
]

strip_states: List[StripState] = []


def _init_strip_states():
    """Initialize strip states with default mode."""
    global strip_states
    strip_states = [
        StripState(
            strip=strip,
            mode=strip.modes["NightRider"],
            delay=DEFAULT_DELAY,
            last_update=time.time(),
        )
        for strip in strips
    ]


_init_strip_states()


# Debug mode control
_debug_mode = False
_loop_control = {
    "state": "running",  # "running" or "paused"
    "iterations_remaining": -1,  # -1 means unlimited
    "event": threading.Event(),
}
_loop_control["event"].set()  # Start in running state


def set_debug_mode(enabled):
    global _debug_mode
    _debug_mode = enabled


def get_debug_mode():
    return _debug_mode


def loop_control(iterations=None, next_state=None):
    """Control the loop for testing purposes. Only works in debug mode.

    If iterations is provided, run that many iterations then auto-pause.
    If only next_state is provided, apply it immediately.
    """
    if not _debug_mode:
        return {"error": "debug mode not enabled"}

    if iterations is not None:
        # Run N iterations then pause (next_state is ignored when stepping)
        _loop_control["iterations_remaining"] = iterations
        _loop_control["state"] = "running"
        _loop_control["event"].set()
    elif next_state == "pause":
        _loop_control["state"] = "paused"
        _loop_control["event"].clear()
    elif next_state == "running":
        _loop_control["state"] = "running"
        _loop_control["iterations_remaining"] = -1
        _loop_control["event"].set()

    return {
        "state": _loop_control["state"],
        "iterations_remaining": _loop_control["iterations_remaining"],
    }


def get_loop_state():
    """Get current loop state."""
    return {
        "state": _loop_control["state"],
        "iterations_remaining": _loop_control["iterations_remaining"],
        "debug_mode": _debug_mode,
    }


def loop():
    """Main animation loop with per-strip timing."""
    global _loop_control
    while True:
        # Debug mode: wait if paused
        if _debug_mode:
            _loop_control["event"].wait()

        now = time.time()
        min_wait = float('inf')

        # Update each strip based on its own delay
        for state in strip_states:
            elapsed = now - state.last_update
            if elapsed >= state.delay:
                state.mode.update()
                state.last_update = now
            remaining = state.delay - (now - state.last_update)
            min_wait = min(min_wait, max(0, remaining))

        # Sleep until next strip needs update
        if min_wait > 0 and min_wait != float('inf'):
            time.sleep(min_wait)
        elif min_wait == float('inf'):
            # Fallback if no strips
            time.sleep(DEFAULT_DELAY)

        # Debug mode: handle iteration counting
        if _debug_mode and _loop_control["iterations_remaining"] > 0:
            _loop_control["iterations_remaining"] -= 1
            if _loop_control["iterations_remaining"] == 0:
                _loop_control["state"] = "paused"
                _loop_control["event"].clear()


# --- Global functions (apply to ALL strips for backward compatibility) ---

def set_mode(m):
    """Set mode for ALL strips."""
    for state in strip_states:
        state.mode = state.strip.modes[m]
        if hasattr(state.mode, "load_cb"):
            state.mode.load_cb({"set_delay": lambda d: set_strip_delay(state.strip.dev_id, d)})


def get_current_mode():
    """Get current mode (from first strip for backward compatibility)."""
    if strip_states:
        return strip_states[0].get_mode_name()
    return "Unknown"


def get_modes():
    """Get available modes."""
    return strips[0].modes


def get_opts():
    """Get options (from first strip for backward compatibility)."""
    if strip_states:
        return strip_states[0].get_opts()
    return {}


def set_opts(val):
    """Set options for ALL strips."""
    for state in strip_states:
        state.set_opts(val)


def get_delay():
    """Get delay (from first strip for backward compatibility)."""
    if strip_states:
        return strip_states[0].delay
    return DEFAULT_DELAY


def set_delay(num):
    """Set delay for ALL strips."""
    for state in strip_states:
        state.delay = num


def get_brightness():
    """Get brightness (from first strip for backward compatibility)."""
    if strip_states:
        return strip_states[0].get_brightness()
    return 255


def set_brightness(num):
    """Set brightness for ALL strips."""
    for state in strip_states:
        state.set_brightness(num)


# --- Per-strip functions ---

def _find_strip_state(strip_id: int) -> Optional[StripState]:
    """Find strip state by ID."""
    for state in strip_states:
        if state.strip.dev_id == strip_id:
            return state
    return None


def get_strip_mode(strip_id: int) -> Optional[str]:
    """Get mode for a specific strip."""
    state = _find_strip_state(strip_id)
    if state:
        return state.get_mode_name()
    return None


def set_strip_mode(strip_id: int, mode_name: str) -> Optional[str]:
    """Set mode for a specific strip."""
    state = _find_strip_state(strip_id)
    if state and mode_name in state.strip.modes:
        state.mode = state.strip.modes[mode_name]
        if hasattr(state.mode, "load_cb"):
            state.mode.load_cb({"set_delay": lambda d: set_strip_delay(strip_id, d)})
        return state.get_mode_name()
    return None


def get_strip_brightness(strip_id: int) -> Optional[int]:
    """Get brightness for a specific strip."""
    state = _find_strip_state(strip_id)
    if state:
        return state.get_brightness()
    return None


def set_strip_brightness(strip_id: int, val: int) -> Optional[int]:
    """Set brightness for a specific strip."""
    state = _find_strip_state(strip_id)
    if state:
        state.set_brightness(val)
        return state.get_brightness()
    return None


def get_strip_delay(strip_id: int) -> Optional[float]:
    """Get delay for a specific strip."""
    state = _find_strip_state(strip_id)
    if state:
        return state.delay
    return None


def set_strip_delay(strip_id: int, val: float) -> Optional[float]:
    """Set delay for a specific strip."""
    state = _find_strip_state(strip_id)
    if state:
        state.delay = val
        return state.delay
    return None


def get_strip_opts(strip_id: int) -> Optional[Dict[str, Any]]:
    """Get options for a specific strip."""
    state = _find_strip_state(strip_id)
    if state:
        return state.get_opts()
    return None


def set_strip_opts(strip_id: int, opts: Dict[str, Any]) -> Optional[Dict[str, Any]]:
    """Set options for a specific strip."""
    state = _find_strip_state(strip_id)
    if state:
        state.set_opts(opts)
        return state.get_opts()
    return None


def get_strips():
    """Return strip configuration for API consumers with per-strip state."""
    result = []
    for state in strip_states:
        strip = state.strip
        result.append({
            "id": strip.dev_id,
            "hostname": strip.UDP_IP,
            "port": strip.UDP_PORT,
            "num_leds": strip.NUM_LEDS,
            "mode": state.get_mode_name(),
            "brightness": state.get_brightness(),
            "delay": state.delay,
        })
    return result


def configure_strip(strip_id, hostname=None, port=None):
    """Configure a strip's network settings (debug mode only).

    This allows tests to redirect UDP packets to localhost listeners.
    """
    if not _debug_mode:
        return {"error": "debug mode not enabled"}

    for state in strip_states:
        strip = state.strip
        if strip.dev_id == strip_id:
            if hostname is not None:
                strip.UDP_IP = hostname
            if port is not None:
                strip.UDP_PORT = port
            return {
                "id": strip.dev_id,
                "hostname": strip.UDP_IP,
                "port": strip.UDP_PORT,
            }

    return {"error": f"strip {strip_id} not found"}


def start_loop():
    _thread.start_new_thread(loop, ())
