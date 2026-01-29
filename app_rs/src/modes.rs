use crate::colors;
use crate::opts::{parse_color, OptValue};
use crate::strip::Strip;
use std::collections::HashMap;

pub trait Mode: Send + Sync {
    fn name(&self) -> &str;
    fn update(&mut self, strip: &mut Strip);
    fn get_opts(&self) -> HashMap<String, OptValue>;
    fn set_opts(&mut self, opts: HashMap<String, OptValue>);
    fn load_cb(&mut self, _set_delay: &dyn Fn(f64)) {}
}

pub struct RainbowCycle {
    colors: Vec<[u8; 3]>,
}

impl RainbowCycle {
    pub fn new(strip: &Strip) -> Self {
        let mut colors = Vec::new();
        let multiplier = 1.0 / strip.num_leds as f64;
        for x in 0..strip.num_leds {
            colors.push(strip.get_rgb(x as f64 * multiplier, 1.0, 1.0));
        }
        Self { colors }
    }
}

impl Mode for RainbowCycle {
    fn name(&self) -> &str {
        "RainbowCycle"
    }

    fn update(&mut self, strip: &mut Strip) {
        for x in 0..strip.num_leds {
            strip.set_led_arr(x, self.colors[x]);
        }
        strip.send();
        // Rotate colors
        let last = self.colors.pop().unwrap();
        self.colors.insert(0, last);
    }

    fn get_opts(&self) -> HashMap<String, OptValue> {
        HashMap::new()
    }

    fn set_opts(&mut self, _opts: HashMap<String, OptValue>) {}
}

pub struct NightRider {
    opts: HashMap<String, OptValue>,
    counter: isize,
    direction: isize,
}

impl NightRider {
    pub fn new() -> Self {
        let mut opts = HashMap::new();
        opts.insert("color".to_string(), OptValue::create_color(colors::PURPLE));
        opts.insert("tail_color".to_string(), OptValue::create_color(colors::BLUE));
        opts.insert("fade".to_string(), OptValue::create_bool(true));
        opts.insert("fill_color".to_string(), OptValue::create_color(colors::BLACK));

        Self {
            opts,
            counter: 0,
            direction: 1,
        }
    }

    fn get_color_opt(&self, key: &str) -> [u8; 3] {
        self.opts
            .get(key)
            .and_then(|v| parse_color(&v.value))
            .unwrap_or(colors::BLACK)
    }

    fn get_bool_opt(&self, key: &str) -> bool {
        self.opts.get(key).and_then(|v| v.value.as_bool()).unwrap_or(false)
    }
}

impl Mode for NightRider {
    fn name(&self) -> &str {
        "NightRider"
    }

    fn update(&mut self, strip: &mut Strip) {
        let fill_color = self.get_color_opt("fill_color");
        let fade = self.get_bool_opt("fade");

        if !fade {
            for x in 0..strip.num_leds {
                strip.set_led_arr(x, fill_color);
            }
        }

        let color = self.get_color_opt("color");
        let tail_color = self.get_color_opt("tail_color");

        for x in 0..strip.num_leds {
            if self.counter == x as isize {
                strip.set_led_arr(x, color);
                if tail_color != colors::BLACK {
                    let tail_pos = (x as isize - self.direction) as usize;
                    if self.counter != 0 && self.counter != (strip.num_leds as isize - 1) && tail_pos < strip.num_leds {
                        strip.set_led_arr(tail_pos, tail_color);
                    }
                }
            } else if fade {
                if let Some(rgb) = strip.get_led(x) {
                    strip.set_led_arr(x, strip.vol(rgb, 0.60));
                }
            }
        }

        self.counter += self.direction;
        if self.counter >= strip.num_leds as isize {
            self.counter = strip.num_leds as isize - 1;
            self.direction = -1;
        }
        if self.counter < 0 {
            self.counter = 1;
            self.direction = 1;
        }

        strip.send();
    }

    fn get_opts(&self) -> HashMap<String, OptValue> {
        self.opts.clone()
    }

    fn set_opts(&mut self, opts: HashMap<String, OptValue>) {
        for (key, val) in opts {
            if self.opts.contains_key(&key) {
                self.opts.insert(key, val);
            }
        }
    }
}

pub struct Collider {
    opts: HashMap<String, OptValue>,
    counter_a: isize,
    direction_a: isize,
    counter_b: isize,
    direction_b: isize,
    collision: Vec<usize>,
    collision_stren: i32,
}

impl Collider {
    pub fn new(strip: &Strip) -> Self {
        let mut opts = HashMap::new();
        opts.insert("color_a".to_string(), OptValue::create_color(colors::PURPLE));
        opts.insert("color_b".to_string(), OptValue::create_color(colors::BLUE));
        opts.insert("collision_decay".to_string(), OptValue::create_int(7));

        Self {
            opts,
            counter_a: 0,
            direction_a: 1,
            counter_b: strip.num_leds as isize,
            direction_b: -1,
            collision: Vec::new(),
            collision_stren: 100,
        }
    }

    fn get_color_opt(&self, key: &str) -> [u8; 3] {
        self.opts
            .get(key)
            .and_then(|v| parse_color(&v.value))
            .unwrap_or(colors::BLACK)
    }

    fn get_int_opt(&self, key: &str) -> i64 {
        self.opts.get(key).and_then(|v| v.value.as_i64()).unwrap_or(0)
    }

    fn _update(&mut self, strip: &mut Strip, counter: isize, direction: isize, color: [u8; 3]) -> (isize, isize) {
        for x in 0..strip.num_leds {
            if counter == x as isize {
                strip.set_led_arr(x, color);
            }
        }

        let mut new_counter = counter + direction;
        let mut new_direction = direction;

        if new_counter >= strip.num_leds as isize {
            new_counter = strip.num_leds as isize;
            new_direction = -1;
        }
        if new_counter < 0 {
            new_counter = 1;
            new_direction = 1;
        }

        (new_counter, new_direction)
    }
}

impl Mode for Collider {
    fn name(&self) -> &str {
        "Collider"
    }

    fn update(&mut self, strip: &mut Strip) {
        strip.solid(colors::BLACK);

        if !self.collision.is_empty() {
            let stren = self.collision_stren as f64 / 100.0;
            let color = [
                (colors::RED[0] as f64 * stren) as u8,
                (colors::RED[1] as f64 * stren) as u8,
                (colors::RED[2] as f64 * stren) as u8,
            ];
            for i in &self.collision {
                strip.set_led_arr(*i, color);
            }
            self.collision_stren -= self.get_int_opt("collision_decay") as i32;
            if self.collision_stren < 0 {
                self.collision.clear();
                self.collision_stren = 100;
            }
        }

        let color_a = self.get_color_opt("color_a");
        let color_b = self.get_color_opt("color_b");

        let (ca, da) = self._update(strip, self.counter_a, self.direction_a, color_a);
        self.counter_a = ca;
        self.direction_a = da;

        let (cb, db) = self._update(strip, self.counter_b, self.direction_b, color_b);
        self.counter_b = cb;
        self.direction_b = db;

        if self.counter_a >= self.counter_b {
            self.direction_a *= -1;
            self.direction_b *= -1;
            self.collision = vec![self.counter_a as usize, self.counter_b as usize];
        }

        strip.send();
    }

    fn get_opts(&self) -> HashMap<String, OptValue> {
        self.opts.clone()
    }

    fn set_opts(&mut self, opts: HashMap<String, OptValue>) {
        for (key, val) in opts {
            if self.opts.contains_key(&key) {
                self.opts.insert(key, val);
            }
        }
    }
}

pub struct Christmas {
    arr: Vec<[u8; 3]>,
    pool_idx: usize,
}

impl Christmas {
    pub fn new() -> Self {
        Self {
            arr: vec![colors::RED, colors::GREEN],
            pool_idx: 0,
        }
    }
}

impl Mode for Christmas {
    fn name(&self) -> &str {
        "Christmas"
    }

    fn update(&mut self, strip: &mut Strip) {
        if (self.arr.len() + strip.num_leds) % 2 == 0 {
            self.pool_idx = (self.pool_idx + 1) % self.arr.len();
        }

        for x in 0..strip.num_leds {
            strip.set_led_arr(x, self.arr[self.pool_idx]);
            self.pool_idx = (self.pool_idx + 1) % self.arr.len();
        }
        strip.send();
    }

    fn get_opts(&self) -> HashMap<String, OptValue> {
        HashMap::new()
    }

    fn set_opts(&mut self, _opts: HashMap<String, OptValue>) {}
}

pub struct MardiGras {
    arr: Vec<[u8; 3]>,
    pool_idx: usize,
}

impl MardiGras {
    pub fn new() -> Self {
        Self {
            arr: vec![colors::PURPLE, colors::GREEN, colors::YELLOW],
            pool_idx: 0,
        }
    }
}

impl Mode for MardiGras {
    fn name(&self) -> &str {
        "MardiGras"
    }

    fn update(&mut self, strip: &mut Strip) {
        if (self.arr.len() + strip.num_leds) % 2 == 0 {
            self.pool_idx = (self.pool_idx + 1) % self.arr.len();
        }

        for x in 0..strip.num_leds {
            strip.set_led_arr(x, self.arr[self.pool_idx]);
            self.pool_idx = (self.pool_idx + 1) % self.arr.len();
        }
        strip.send();
    }

    fn get_opts(&self) -> HashMap<String, OptValue> {
        HashMap::new()
    }

    fn set_opts(&mut self, _opts: HashMap<String, OptValue>) {}
}

pub struct ArrGeeBee {
    arr: Vec<[u8; 3]>,
    pool_idx: usize,
}

impl ArrGeeBee {
    pub fn new() -> Self {
        Self {
            arr: vec![colors::RED, colors::GREEN, colors::BLUE],
            pool_idx: 0,
        }
    }
}

impl Mode for ArrGeeBee {
    fn name(&self) -> &str {
        "ArrGeeBee"
    }

    fn update(&mut self, strip: &mut Strip) {
        if (self.arr.len() + strip.num_leds) % 2 == 0 {
            self.pool_idx = (self.pool_idx + 1) % self.arr.len();
        }

        for x in 0..strip.num_leds {
            strip.set_led_arr(x, self.arr[self.pool_idx]);
            self.pool_idx = (self.pool_idx + 1) % self.arr.len();
        }
        strip.send();
    }

    fn get_opts(&self) -> HashMap<String, OptValue> {
        HashMap::new()
    }

    fn set_opts(&mut self, _opts: HashMap<String, OptValue>) {}
}

pub struct Sparkle {
    opts: HashMap<String, OptValue>,
    arr: [u8; 3],
}

impl Sparkle {
    pub fn new() -> Self {
        let mut opts = HashMap::new();
        opts.insert("low".to_string(), OptValue::create_int(0));
        opts.insert("high".to_string(), OptValue::create_int(255));
        opts.insert("r_on".to_string(), OptValue::create_bool(true));
        opts.insert("g_on".to_string(), OptValue::create_bool(false));
        opts.insert("b_on".to_string(), OptValue::create_bool(true));
        opts.insert("decay".to_string(), OptValue::create_float(0.92));

        Self { opts, arr: [0, 0, 0] }
    }

    fn get_int_opt(&self, key: &str) -> i64 {
        self.opts.get(key).and_then(|v| v.value.as_i64()).unwrap_or(0)
    }

    fn get_bool_opt(&self, key: &str) -> bool {
        self.opts.get(key).and_then(|v| v.value.as_bool()).unwrap_or(false)
    }

    fn get_float_opt(&self, key: &str) -> f64 {
        self.opts.get(key).and_then(|v| v.value.as_f64()).unwrap_or(0.0)
    }

    fn get_arr(&mut self) -> [u8; 3] {
        use std::time::{SystemTime, UNIX_EPOCH};

        // Simple RNG using timestamp
        let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64;

        let low = self.get_int_opt("low") as u8;
        let high = self.get_int_opt("high") as u8;

        if self.get_bool_opt("r_on") {
            self.arr[0] = (seed % ((high - low + 1) as u64)) as u8 + low;
        } else {
            self.arr[0] = 0;
        }
        if self.get_bool_opt("g_on") {
            self.arr[1] = ((seed >> 8) % ((high - low + 1) as u64)) as u8 + low;
        } else {
            self.arr[1] = 0;
        }
        if self.get_bool_opt("b_on") {
            self.arr[2] = ((seed >> 16) % ((high - low + 1) as u64)) as u8 + low;
        } else {
            self.arr[2] = 0;
        }

        self.arr
    }
}

impl Mode for Sparkle {
    fn name(&self) -> &str {
        "Sparkle"
    }

    fn update(&mut self, strip: &mut Strip) {
        let decay = self.get_float_opt("decay");
        let threshold = strip.num_leds * 3;

        for x in 0..strip.num_leds {
            // Simple pseudo-random
            use std::time::{SystemTime, UNIX_EPOCH};
            let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64;

            if (seed + x as u64) % (threshold as u64 + 1) == 0 {
                strip.set_led_arr(x, self.get_arr());
            } else {
                if let Some(rgb) = strip.get_led(x) {
                    strip.set_led_arr(
                        x,
                        [
                            (rgb[0] as f64 * decay) as u8,
                            (rgb[1] as f64 * decay) as u8,
                            (rgb[2] as f64 * decay) as u8,
                        ],
                    );
                }
            }
        }
        strip.send();
    }

    fn get_opts(&self) -> HashMap<String, OptValue> {
        self.opts.clone()
    }

    fn set_opts(&mut self, opts: HashMap<String, OptValue>) {
        for (key, val) in opts {
            if self.opts.contains_key(&key) {
                self.opts.insert(key, val);
            }
        }
    }
}

pub struct Breathe {
    opts: HashMap<String, OptValue>,
    counter: i64,
    direction: i64,
}

impl Breathe {
    pub fn new() -> Self {
        let mut opts = HashMap::new();
        opts.insert("r".to_string(), OptValue::create_int(255));
        opts.insert("g".to_string(), OptValue::create_int(0));
        opts.insert("b".to_string(), OptValue::create_int(255));
        opts.insert("low".to_string(), OptValue::create_int(0));
        opts.insert("high".to_string(), OptValue::create_int(255));

        Self {
            opts,
            counter: 0,
            direction: 1,
        }
    }

    fn get_int_opt(&self, key: &str) -> i64 {
        self.opts.get(key).and_then(|v| v.value.as_i64()).unwrap_or(0)
    }
}

impl Mode for Breathe {
    fn name(&self) -> &str {
        "Breathe"
    }

    fn update(&mut self, strip: &mut Strip) {
        let r = self.get_int_opt("r") as u8;
        let g = self.get_int_opt("g") as u8;
        let b = self.get_int_opt("b") as u8;
        let low = self.get_int_opt("low");
        let high = self.get_int_opt("high");

        strip.set_brightness(self.counter as u8);
        strip.solid([r, g, b]);

        if self.counter >= high {
            self.direction = -1;
        } else if self.counter <= low {
            self.direction = 1;
        }
        self.counter += self.direction;
    }

    fn get_opts(&self) -> HashMap<String, OptValue> {
        self.opts.clone()
    }

    fn set_opts(&mut self, opts: HashMap<String, OptValue>) {
        for (key, val) in opts {
            if self.opts.contains_key(&key) {
                self.opts.insert(key, val);
            }
        }
    }
}

pub struct Solid {
    opts: HashMap<String, OptValue>,
}

impl Solid {
    pub fn new() -> Self {
        let mut opts = HashMap::new();
        opts.insert("color".to_string(), OptValue::create_color(colors::YELLOW));

        Self { opts }
    }

    fn get_color_opt(&self, key: &str) -> [u8; 3] {
        self.opts
            .get(key)
            .and_then(|v| parse_color(&v.value))
            .unwrap_or(colors::BLACK)
    }
}

impl Mode for Solid {
    fn name(&self) -> &str {
        "Solid"
    }

    fn update(&mut self, strip: &mut Strip) {
        let color = self.get_color_opt("color");
        strip.solid(color);
    }

    fn get_opts(&self) -> HashMap<String, OptValue> {
        self.opts.clone()
    }

    fn set_opts(&mut self, opts: HashMap<String, OptValue>) {
        for (key, val) in opts {
            if self.opts.contains_key(&key) {
                self.opts.insert(key, val);
            }
        }
    }

    fn load_cb(&mut self, set_delay: &dyn Fn(f64)) {
        set_delay(0.250);
    }
}

pub struct White;

impl White {
    pub fn new() -> Self {
        Self
    }
}

impl Mode for White {
    fn name(&self) -> &str {
        "White"
    }

    fn update(&mut self, strip: &mut Strip) {
        strip.solid([255, 255, 255]);
    }

    fn get_opts(&self) -> HashMap<String, OptValue> {
        HashMap::new()
    }

    fn set_opts(&mut self, _opts: HashMap<String, OptValue>) {}

    fn load_cb(&mut self, set_delay: &dyn Fn(f64)) {
        set_delay(0.250);
    }
}

pub struct Off;

impl Off {
    pub fn new() -> Self {
        Self
    }
}

impl Mode for Off {
    fn name(&self) -> &str {
        "Off"
    }

    fn update(&mut self, strip: &mut Strip) {
        strip.solid(colors::BLACK);
    }

    fn get_opts(&self) -> HashMap<String, OptValue> {
        HashMap::new()
    }

    fn set_opts(&mut self, _opts: HashMap<String, OptValue>) {}

    fn load_cb(&mut self, set_delay: &dyn Fn(f64)) {
        set_delay(0.250);
    }
}

pub fn create_mode(name: &str, strip: &Strip) -> Box<dyn Mode> {
    match name {
        "NightRider" => Box::new(NightRider::new()),
        "RainbowCycle" => Box::new(RainbowCycle::new(strip)),
        "Collider" => Box::new(Collider::new(strip)),
        "Christmas" => Box::new(Christmas::new()),
        "MardiGras" => Box::new(MardiGras::new()),
        "ArrGeeBee" => Box::new(ArrGeeBee::new()),
        "Sparkle" => Box::new(Sparkle::new()),
        "Breathe" => Box::new(Breathe::new()),
        "Solid" => Box::new(Solid::new()),
        "White" => Box::new(White::new()),
        "Off" => Box::new(Off::new()),
        _ => Box::new(NightRider::new()),
    }
}

pub fn get_available_modes() -> Vec<String> {
    vec![
        "NightRider".to_string(),
        "RainbowCycle".to_string(),
        "Collider".to_string(),
        "Christmas".to_string(),
        "MardiGras".to_string(),
        "ArrGeeBee".to_string(),
        "Sparkle".to_string(),
        "Breathe".to_string(),
        "Solid".to_string(),
        "White".to_string(),
        "Off".to_string(),
    ]
}
