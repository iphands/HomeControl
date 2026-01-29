import _thread
import copy
import threading
from strip_ctl import Strip
from time import sleep
from datetime import datetime

DELAY = 0.0250

strips = [
    Strip(1, "esp32c6-00.lan", 67),
    Strip(2, "esp32c6-01.lan", 83),
]

modes = [strip.modes["NightRider"] for strip in strips]

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
    global _loop_control
    while True:
        # Debug mode: wait if paused
        if _debug_mode:
            _loop_control["event"].wait()

        a = datetime.now()
        for mode in modes:
            mode.update()
        b = datetime.now()
        c = (b - a).microseconds / 1000000.0
        if DELAY > c:
            sleep(DELAY - c)

        # Debug mode: handle iteration counting
        if _debug_mode and _loop_control["iterations_remaining"] > 0:
            _loop_control["iterations_remaining"] -= 1
            if _loop_control["iterations_remaining"] == 0:
                _loop_control["state"] = "paused"
                _loop_control["event"].clear()


def set_mode(m):
    global modes
    modes = [strip.modes[m] for strip in strips]
    for mode in modes:
        if hasattr(mode, "load_cb"):
            mode.load_cb({"set_delay": set_delay})


def get_current_mode():
    return modes[0].name


def get_modes():
    return strips[0].modes


def get_opts():
    return modes[0].get_opts()


def set_opts(val):
    for mode in modes:
        mode.set_opts(copy.deepcopy(val))


def get_delay():
    return DELAY


def set_delay(num):
    global DELAY
    DELAY = num


def get_brightness():
    return strips[0].get_brightness()


def set_brightness(num):
    for strip in strips:
        strip.set_brightness(num)


def get_strips():
    """Return strip configuration for API consumers."""
    return [
        {
            "id": strip.dev_id,
            "hostname": strip.UDP_IP,
            "port": strip.UDP_PORT,
            "num_leds": strip.NUM_LEDS,
        }
        for strip in strips
    ]


def configure_strip(strip_id, hostname=None, port=None):
    """Configure a strip's network settings (debug mode only).

    This allows tests to redirect UDP packets to localhost listeners.
    """
    if not _debug_mode:
        return {"error": "debug mode not enabled"}

    for strip in strips:
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
