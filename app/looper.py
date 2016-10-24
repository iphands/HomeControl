import thread
import strip_ctl as strip
from time import sleep
from datetime import datetime

DELAY = 0.0125
mode = strip.modes['NightRider']

def loop():
    while True:
        a = datetime.now()
        try:
            mode.update()
        except:
            pass
        b = datetime.now()
        c = (b - a).microseconds / 1000000.0
        if DELAY > c:
            sleep(DELAY - c)

def set_mode(m):
    global mode
    mode = strip.modes[m]
    if hasattr(mode, 'load_cb'):
        mode.load_cb({
            'set_delay': set_delay
        })

def get_current_mode():
    return mode.name

def get_modes():
    return strip.modes

def get_opts():
    return mode.get_opts()

def set_opts(input):
    global mode
    mode.set_opts(input)

def get_delay():
    return DELAY

def set_delay(num):
    global DELAY
    DELAY = num

def get_brightness():
    return strip.get_brightness()

def set_brightness(num):
    return strip.set_brightness(num)

def start_loop():
    thread.start_new_thread(loop, ())
