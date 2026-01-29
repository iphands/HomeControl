import _thread
from strip_ctl import Strip
from time import sleep
from datetime import datetime

DELAY = 0.0250

strips = [
    Strip(1, "esp32c6-00.lan", 67),
    Strip(2, "esp32c6-01.lan", 83),
]

modes = [
    strips[0].modes["NightRider"],
    strips[1].modes["NightRider"],
]

def loop():
    while True:
        a = datetime.now()
        # mode.say_name()
        modes[0].update()
        modes[1].update()
        b = datetime.now()
        c = (b - a).microseconds / 1000000.0
        if DELAY > c:
            sleep(DELAY - c)


def set_mode(m):
    global modes
    modes[1] = strips[1].modes[m]
    if hasattr(modes[1], "load_cb"):
        modes[1].load_cb({"set_delay": set_delay})

    modes[0] = strips[0].modes[m]
    if hasattr(modes[0], "load_cb"):
        modes[0].load_cb({"set_delay": set_delay})


def get_current_mode():
    return modes[0].name


def get_modes():
    return strips[0].modes


def get_opts():
    return modes[0].get_opts()


def set_opts(val):
    global modes
    modes[1].set_opts(val)
    # modes[0].set_opts(val)


def get_delay():
    return DELAY


def set_delay(num):
    global DELAY
    DELAY = num


def get_brightness():
    return strips[0].get_brightness()


def set_brightness(num):
    strips[1].set_brightness(num)
    return strips[0].set_brightness(num)


def start_loop():
    _thread.start_new_thread(loop, ())
