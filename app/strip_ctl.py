#!/usr/bin/python
import socket, math, colorsys
import colors as colors
import opts as options
from dotmap import DotMap
from itertools import cycle
from time import sleep
from random import randint


# modes = {}

class Strip():
    def __init__(self, dev_id, hostname, num_leds):
        self.PROTOCOL_SKIP = 5
        self.SEQ = 0

        self.dev_id = dev_id

        self.modes = {}
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)

        self.UDP_IP = hostname
        self.UDP_PORT = 4210

        self.NUM_LEDS = num_leds
        self.TOTAL = (self.NUM_LEDS * 3) + self.PROTOCOL_SKIP

        self.LEDS = [0] * ((self.NUM_LEDS * 3) + self.PROTOCOL_SKIP)
        self.LEDS[0] = 1
        self.LEDS[1] = 0
        self.LEDS[2] = 255
        self.LEDS[3] = self.NUM_LEDS
        self.LEDS[4] = dev_id

        # init the modes for now
        NightRider(self)
        RainbowCycle(self)
        Collider(self)
        Christmas(self)
        MardiGras(self)
        ArrGeeBee(self)
        Sparkle(self)
        Breathe(self)
        Solid(self)
        White(self)
        Off(self)


    def get_brightness(self):
        return self.LEDS[2]


    def set_brightness(self, val):
        self.LEDS[2] = val


    def set_led(self, led, r, g, b, dbg=False):
        if led > -1:
            self.LEDS[(led * 3) + self.PROTOCOL_SKIP + 0] = r
            self.LEDS[(led * 3) + self.PROTOCOL_SKIP + 1] = g
            self.LEDS[(led * 3) + self.PROTOCOL_SKIP + 2] = b
            if dbg:
                print("Setting %d to %d, %d, %d" % (led, r, g, b))


    def set_led_arr(self, led, arr, dbg=False):
        self.set_led(led, arr[0], arr[1], arr[2], dbg)


    def get_led(self, led):
        r = self.LEDS[(led * 3) + self.PROTOCOL_SKIP + 0]
        g = self.LEDS[(led * 3) + self.PROTOCOL_SKIP + 1]
        b = self.LEDS[(led * 3) + self.PROTOCOL_SKIP + 2]
        return [r, g, b]


    def vol(self, rgb, pct):
        rgb[0] = rgb[0] * pct
        rgb[1] = rgb[1] * pct
        rgb[2] = rgb[2] * pct
        return rgb


    def get_packet(self):
        self.LEDS[1] = self.SEQ
        t = [int(i) for i in self.LEDS]
        self.SEQ += 1
        if self.SEQ > 255:
            self.SEQ = 0

        assert len(t) == self.TOTAL
        # if self.dev_id == 2:
        #    print(t)
        return t


    def send(self, delay=0.001):
        self.sock.sendto(bytearray(self.get_packet()), (self.UDP_IP, self.UDP_PORT))
        sleep(delay)


    def simple_walk(self, arr, pool):
        if not (len(arr) + self.NUM_LEDS) % 2:
            next(pool)
        for x in range(0, self.NUM_LEDS):
            self.set_led_arr(x, next(pool))
        self.send()


    def rainbow_solid():
        while True:
            for i in range(0, 1000):
                x = i / 1000.0
                self.solid(get_rgb(x, 1, 1))
                sleep(0.01)


    def get_rgb(self, h, s, v):
        arr = colorsys.hsv_to_rgb(h, s, v)
        return [arr[0] * 255, arr[1] * 255, arr[2] * 255]


    def solid(self, arr):
        for x in range(0, self.NUM_LEDS):
            self.set_led_arr(x, arr)
        self.send()


class Mode:
    def __init__(self, name, strip, opts):
        self.opts = DotMap(opts)
        self.name = name
        self.strip = strip
        strip.modes[name] = self

    def say_name(self):
        print(f"MODE: {self.name}")

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
    def __init__(self, strip):
        Mode.__init__(self, self.__class__.__name__, strip, {})
        self.colors = []
        self.multiplier = 1.0 / self.strip.NUM_LEDS
        for x in range(0, self.strip.NUM_LEDS):
            self.colors.append(self.strip.get_rgb((x * self.multiplier), 1, 1))

    def update(self):
        for x in range(0, self.strip.NUM_LEDS):
            self.strip.set_led_arr(x, self.colors[x])
        self.strip.send()
        self.colors.insert(0, self.colors.pop())


class NightRider(Mode):
    def __init__(self, strip):
        Mode.__init__(
            self,
            self.__class__.__name__,
            strip,
            {
                "color": options.create_color(colors.PURPLE),
                "tail_color": options.create_color(colors.BLUE),
                "fade": options.create_bool(True),
                "fill_color": options.create_color(colors.BLACK),
            },
        )
        self.counter = 0
        self.direction = 1

    def update(self):
        if not self.opts.fade.val:
            for x in range(0, self.strip.NUM_LEDS):
                self.strip.set_led_arr(x, self.opts.fill_color.val)
        for x in range(0, self.strip.NUM_LEDS):
            if self.counter == x:
                self.strip.set_led_arr(x, self.opts.color.val)
                if self.opts.tail_color.val:
                    if self.counter != 0 and self.counter != (self.strip.NUM_LEDS - 1):
                        self.strip.set_led_arr((x - self.direction), self.opts.tail_color.val)
            elif self.opts.fade.val:
                self.strip.set_led_arr(x, self.strip.vol(self.strip.get_led(x), 0.60))
        self.counter += self.direction
        if self.counter >= self.strip.NUM_LEDS:
            self.counter = self.strip.NUM_LEDS - 1
            self.direction = -1
        if self.counter < 0:
            self.counter = 1
            self.direction = 1
        self.strip.send()


class Collider(Mode):
    def __init__(self, strip):
        Mode.__init__(
            self,
            self.__class__.__name__,
            strip,
            {
                "color_a": options.create_color(colors.PURPLE),
                "color_b": options.create_color(colors.BLUE),
                "collision_decay": options.create_int(7),
            },
        )
        self.counter_a = 0
        self.direction_a = 1
        self.counter_b = self.strip.NUM_LEDS
        self.direction_b = -1
        self.collision = []
        self.collision_stren = 100

    def update(self):
        self.strip.solid([0, 0, 0])
        if len(self.collision):
            stren = self.collision_stren / 100
            color = [int(i * stren) for i in colors.RED]
            for i in self.collision:
                self.strip.set_led_arr(i, color)
                self.collision_stren -= self.opts.collision_decay.val
            if self.collision_stren < 0:
                self.collision = []
                self.collision_stren = 100

        MID = int(self.strip.NUM_LEDS / 2)
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
        self.strip.send()

    def _update(self, counter, direction, color):
        for x in range(0, self.strip.NUM_LEDS):
            if counter == x:
                self.strip.set_led_arr(x, color)
        counter += direction
        if counter >= self.strip.NUM_LEDS:
            counter = self.strip.NUM_LEDS
            direction = -1
        if counter < 0:
            counter = 1
            direction = 1
        return counter, direction


class Christmas(Mode):
    def __init__(self, strip):
        Mode.__init__(self, self.__class__.__name__, strip, {})
        self.arr = [colors.RED, colors.GREEN]
        self.pool = cycle(self.arr)

    def update(self):
        self.strip.simple_walk(self.arr, self.pool)


class MardiGras(Mode):
    def __init__(self, strip):
        Mode.__init__(self, self.__class__.__name__, strip, {})
        self.arr = [colors.PURPLE, colors.GREEN, colors.YELLOW]
        self.pool = cycle(self.arr)

    def update(self):
        self.strip.simple_walk(self.arr, self.pool)


class ArrGeeBee(Mode):
    def __init__(self, strip):
        Mode.__init__(self, self.__class__.__name__, strip, {})
        self.arr = [colors.RED, colors.GREEN, colors.BLUE]
        self.pool = cycle(self.arr)

    def update(self):
        self.strip.simple_walk(self.arr, self.pool)


class Sparkle(Mode):
    def __init__(self, strip):
        Mode.__init__(
            self,
            self.__class__.__name__,
            strip,
            {
                "low": options.create_int(0),
                "high": options.create_int(255),
                "r_on": options.create_bool(True),
                "g_on": options.create_bool(False),
                "b_on": options.create_bool(True),
                "decay": options.create_float(0.92),
            },
        )
        self.arr = [0, 0, 0]

    def get_arr(self):
        if self.opts.r_on.val:
            self.arr[0] = randint(self.opts.low.val, self.opts.high.val)
        else:
            self.arr[0] = 0
        if self.opts.g_on.val:
            self.arr[1] = randint(self.opts.low.val, self.opts.high.val)
        else:
            self.arr[1] = 0
        if self.opts.b_on.val:
            self.arr[2] = randint(self.opts.low.val, self.opts.high.val)
        else:
            self.arr[2] = 0
        return self.arr

    def update(self):
        for x in range(0, self.strip.NUM_LEDS):
            if randint(0, (self.strip.NUM_LEDS * 3)) == 0:
                self.strip.set_led_arr(x, self.get_arr())
            else:
                self.strip.set_led_arr(x, self.strip.vol(self.strip.get_led(x), self.opts.decay.val))
        self.strip.send()


class Breathe(Mode):
    def __init__(self, strip):
        Mode.__init__(
            self,
            self.__class__.__name__,
            strip,
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
        self.strip.set_brightness(self.counter)
        self.strip.solid([self.opts.r, self.opts.g, self.opts.b])
        if self.counter >= self.opts.high:
            self.direction = -1
        elif self.counter <= self.opts.low:
            self.direction = 1
        self.counter += self.direction


class Solid(Mode):
    def __init__(self, strip):
        Mode.__init__(
            self,
            self.__class__.__name__,
            strip,
            {"color": options.create_color(colors.YELLOW)},
        )

    def update(self):
        self.strip.solid(self.opts.color.val)

    def load_cb(self, d):
        d["set_delay"](0.250)


class White(Mode):
    def __init__(self, strip):
        Mode.__init__(self, self.__class__.__name__, strip, {})

    def update(self):
        self.strip.solid([255, 255, 255])

    def load_cb(self, d):
        d["set_delay"](0.250)


class Off(Mode):
    def __init__(self, strip):
        Mode.__init__(self, self.__class__.__name__, strip, {})

    def update(self):
        self.strip.solid([0, 0, 0])

    def load_cb(self, d):
        d["set_delay"](0.250)
