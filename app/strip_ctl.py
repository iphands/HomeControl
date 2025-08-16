#!/usr/bin/python
import socket, math, colorsys
import colors as colors
import opts as options
from dotmap import DotMap
from itertools import cycle
from time import sleep
from random import randint

UDP_IP = "esp32c6-00.lan"
UDP_PORT = 4210
PROTOCOL_SKIP = 3
NUM_LEDS = 67

modes = {}

sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
# LEDS = list(range((NUM_LEDS * 3) + PROTOCOL_SKIP))
LEDS = [0] * ((NUM_LEDS * 3) + PROTOCOL_SKIP)
LEDS[0] = 1
LEDS[1] = 0
LEDS[2] = 255
SEQ = 0


def get_brightness():
    return LEDS[2]


def set_brightness(val):
    LEDS[2] = val


def set_led(led, r, g, b, dbg=False):
    if led > -1:
        LEDS[(led * 3) + PROTOCOL_SKIP + 0] = r
        LEDS[(led * 3) + PROTOCOL_SKIP + 1] = g
        LEDS[(led * 3) + PROTOCOL_SKIP + 2] = b
        if dbg:
            print("Setting %d to %d, %d, %d" % (led, r, g, b))


def set_led_arr(led, arr, dbg=False):
    set_led(led, arr[0], arr[1], arr[2], dbg)


def get_led(led):
    r = LEDS[(led * 3) + PROTOCOL_SKIP + 0]
    g = LEDS[(led * 3) + PROTOCOL_SKIP + 1]
    b = LEDS[(led * 3) + PROTOCOL_SKIP + 2]
    return [r, g, b]


def vol(rgb, pct):
    rgb[0] = rgb[0] * pct
    rgb[1] = rgb[1] * pct
    rgb[2] = rgb[2] * pct
    return rgb


def get_packet():
    global SEQ
    LEDS[1] = SEQ
    t = [int(i) for i in LEDS]
    SEQ += 1
    if SEQ > 255:
        SEQ = 0
    return t


def send(delay=0.001):
    global SEQ
    sock.sendto(bytearray(get_packet()), (UDP_IP, UDP_PORT))
    sleep(delay)


def simple_walk(arr, pool):
    if not (len(arr) + NUM_LEDS) % 2:
        next(pool)
    for x in range(0, NUM_LEDS):
        set_led_arr(x, next(pool))
    send()


def rainbow_solid():
    while True:
        for i in range(0, 1000):
            x = i / 1000.0
            solid(get_rgb(x, 1, 1))
            sleep(0.01)


def get_rgb(h, s, v):
    arr = colorsys.hsv_to_rgb(h, s, v)
    return [arr[0] * 255, arr[1] * 255, arr[2] * 255]


def solid(arr):
    for x in range(0, NUM_LEDS):
        set_led_arr(x, arr)
    send()


class Mode:
    def __init__(self, name, opts):
        self.opts = DotMap(opts)
        self.name = name
        modes[name] = self

    def get_opts(self):
        return self.opts.toDict()

    def set_opts(self, newopts):
        orig = self.get_opts()
        for key in newopts:
            if key in orig:
                if newopts[key]["type"] == "color":
                    orig[key] = options.set_color(newopts[key])
                    continue
                orig[key] = newopts[key]
        self.opts = DotMap(orig)


class RainbowCycle(Mode):
    def __init__(self):
        Mode.__init__(self, self.__class__.__name__, {})
        self.colors = []
        self.multiplier = 1.0 / NUM_LEDS
        for x in range(0, NUM_LEDS):
            self.colors.append(get_rgb((x * self.multiplier), 1, 1))

    def update(self):
        for x in range(0, NUM_LEDS):
            set_led_arr(x, self.colors[x])
        send()
        self.colors.insert(0, self.colors.pop())


class NightRider(Mode):
    def __init__(self):
        Mode.__init__(
            self,
            self.__class__.__name__,
            {
                "color": options.create_color(colors.PURPLE),
                "tail_color": options.create_color(colors.BLUE),
                "fade": options.create_bool(True),
                # "fill_color": options.create_color(colors.BLACK),
            },
        )
        self.counter = 0
        self.direction = 1

    def update(self):
        if not self.opts.fade.val:
            for x in range(0, NUM_LEDS):
                set_led_arr(x, self.opts.fill_color.val)
        for x in range(0, NUM_LEDS):
            if self.counter == x:
                set_led_arr(x, self.opts.color.val)
                if self.opts.tail_color.val:
                    if self.counter != 0 and self.counter != (NUM_LEDS - 1):
                        set_led_arr((x - self.direction), self.opts.tail_color.val)
            elif self.opts.fade.val:
                set_led_arr(x, vol(get_led(x), 0.60))
        self.counter += self.direction
        if self.counter >= NUM_LEDS:
            self.counter = NUM_LEDS - 1
            self.direction = -1
        if self.counter < 0:
            self.counter = 1
            self.direction = 1
        send()


class Collider(Mode):
    def __init__(self):
        Mode.__init__(
            self,
            self.__class__.__name__,
            {
                "color_a": options.create_color(colors.PURPLE),
                "color_b": options.create_color(colors.BLUE),
                "collision_decay": options.create_int(7),
            },
        )
        self.counter_a = 0
        self.direction_a = 1
        self.counter_b = NUM_LEDS
        self.direction_b = -1
        self.collision = []
        self.collision_stren = 100

    def update(self):
        solid([0, 0, 0])
        if len(self.collision):
            stren = self.collision_stren / 100
            color = [int(i * stren) for i in colors.RED]
            for i in self.collision:
                set_led_arr(i, color)
                self.collision_stren -= self.opts.collision_decay.val
            if self.collision_stren < 0:
                self.collision = []
                self.collision_stren = 100

        MID = int(NUM_LEDS / 2)
        self.counter_a, self.direction_a = self._update(
            self.counter_a, self.direction_a, self.opts.color_a.val
        )
        self.counter_b, self.direction_b = self._update(
            self.counter_b, self.direction_b, self.opts.color_b.val
        )
        if self.counter_a >= self.counter_b:
            self.direction_a *= -1
            self.direction_b *= -1
            self.collision = [self.counter_a, self.counter_b]
        send()

    def _update(self, counter, direction, color):
        for x in range(0, NUM_LEDS):
            if counter == x:
                set_led_arr(x, color)
        counter += direction
        if counter >= NUM_LEDS:
            counter = NUM_LEDS
            direction = -1
        if counter < 0:
            counter = 1
            direction = 1
        return counter, direction


class Christmas(Mode):
    def __init__(self):
        Mode.__init__(self, self.__class__.__name__, {})
        self.arr = [colors.RED, colors.GREEN]
        self.pool = cycle(self.arr)

    def update(self):
        simple_walk(self.arr, self.pool)


class MardiGras(Mode):
    def __init__(self):
        Mode.__init__(self, self.__class__.__name__, {})
        self.arr = [colors.PURPLE, colors.GREEN, colors.YELLOW]
        self.pool = cycle(self.arr)

    def update(self):
        simple_walk(self.arr, self.pool)


class ArrGeeBee(Mode):
    def __init__(self):
        Mode.__init__(self, self.__class__.__name__, {})
        self.arr = [colors.RED, colors.GREEN, colors.BLUE]
        self.pool = cycle(self.arr)

    def update(self):
        simple_walk(self.arr, self.pool)


class Sparkle(Mode):
    def __init__(self):
        Mode.__init__(
            self,
            self.__class__.__name__,
            {
                "low": 0,
                "high": 255,
                "r_on": True,
                "g_on": False,
                "b_on": True,
                "decay": 0.92,
            },
        )
        self.arr = [0, 0, 0]

    def get_arr(self):
        if self.opts.r_on:
            self.arr[0] = randint(self.opts.low, self.opts.high)
        if self.opts.g_on:
            self.arr[1] = randint(self.opts.low, self.opts.high)
        if self.opts.b_on:
            self.arr[2] = randint(self.opts.low, self.opts.high)
        return self.arr

    def update(self):
        for x in range(0, NUM_LEDS):
            if randint(0, (NUM_LEDS * 3)) == 0:
                set_led_arr(x, self.get_arr())
            else:
                set_led_arr(x, vol(get_led(x), self.opts.decay))
        send()


class Breathe(Mode):
    def __init__(self):
        Mode.__init__(
            self,
            self.__class__.__name__,
            {
                "r": 255,
                "g": 0,
                "b": 255,
                "low": 0,
                "high": 255,
            },
        )
        self.counter = self.opts.low
        self.direction = 1

    def update(self):
        set_brightness(self.counter)
        solid([self.opts.r, self.opts.g, self.opts.b])
        if self.counter >= self.opts.high:
            self.direction = -1
        elif self.counter <= self.opts.low:
            self.direction = 1
        self.counter += self.direction


class Solid(Mode):
    def __init__(self):
        Mode.__init__(
            self,
            self.__class__.__name__,
            {"color": options.create_color(colors.YELLOW)},
        )

    def update(self):
        solid(self.opts.color.val)

    def load_cb(self, d):
        d["set_delay"](0.250)


class White(Mode):
    def __init__(self):
        Mode.__init__(self, self.__class__.__name__, {})

    def update(self):
        solid([255, 255, 255])

    def load_cb(self, d):
        d["set_delay"](0.250)


class Off(Mode):
    def __init__(self):
        Mode.__init__(self, self.__class__.__name__, {})

    def update(self):
        solid([0, 0, 0])

    def load_cb(self, d):
        d["set_delay"](0.250)


# init the modes for now
NightRider()
RainbowCycle()
Collider()
Christmas()
MardiGras()
ArrGeeBee()
Sparkle()
Breathe()
Solid()
White()
Off()
