//! Animation modes for LED strip effects.
//!
//! This module provides a variety of animation modes that can be applied
//! to LED strips. Each mode implements the [`Mode`] trait for consistent
//! interaction with the looper system.
//!
//! # Available Modes
//! - [`NightRider`] - KITT-style scanning light with configurable colors
//! - [`RainbowCycle`] - Rotating rainbow gradient
//! - [`Collider`] - Two particles that collide and create explosion effect
//! - [`Sparkle`] - Random twinkling lights with decay
//! - [`Breathe`] - Pulsing brightness effect
//! - [`Solid`] - Static single color
//! - [`PatternMode`] - Alternating color patterns (Christmas, MardiGras, RGB)
//! - [`FixedColor`] - Static color modes (White, Off)

use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use crate::colors;
use crate::opts::{OptMap, OptValue};
use crate::strip::Strip;

/// Core trait for all animation modes.
///
/// Implementors of this trait define how LEDs are updated each frame
/// and how their configuration options are managed.
pub trait Mode: Send + Sync {
    /// Returns the unique name of this mode.
    fn name(&self) -> &str;

    /// Updates the LED strip for the next animation frame.
    ///
    /// Called by the looper at the configured delay interval.
    fn update(&mut self, strip: &mut Strip);

    /// Returns the current configuration options.
    fn get_opts(&self) -> OptMap;

    /// Applies configuration options to the mode.
    fn set_opts(&mut self, opts: OptMap);

    /// Callback invoked when the mode is loaded.
    ///
    /// Can be used to set initial delay values or perform setup.
    fn on_load(&mut self, _set_delay: &dyn Fn(f64)) {}
}

/// Creates a new mode instance by name.
///
/// # Arguments
/// * `name` - The mode name (case-sensitive)
/// * `strip` - The LED strip for context (LED count, etc.)
///
/// # Returns
/// A boxed mode instance, or NightRider as default if name is unknown.
pub fn create_mode(name: &str, strip: &Strip) -> Box<dyn Mode> {
    match name {
        "NightRider" => Box::new(NightRider::new()),
        "RainbowCycle" => Box::new(RainbowCycle::new(strip)),
        "Collider" => Box::new(Collider::new(strip)),
        "Christmas" => Box::new(PatternMode::christmas()),
        "MardiGras" => Box::new(PatternMode::mardi_gras()),
        "ArrGeeBee" => Box::new(PatternMode::rgb()),
        "Sparkle" => Box::new(Sparkle::new()),
        "Breathe" => Box::new(Breathe::new()),
        "Solid" => Box::new(Solid::new()),
        "White" => Box::new(FixedColor::white()),
        "Off" => Box::new(FixedColor::off()),
        _ => Box::new(NightRider::new()),
    }
}

/// Returns a list of all available mode names.
pub fn available_modes() -> Vec<String> {
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

/// Rotating rainbow gradient animation.
///
/// Pre-computes a full rainbow spectrum and rotates it across the strip.
pub struct RainbowCycle {
    colors: Vec<[u8; 3]>,
}

impl RainbowCycle {
    /// Creates a new rainbow cycle mode for the given strip.
    pub fn new(strip: &Strip) -> Self {
        Self {
            colors: strip.rainbow_colors(),
        }
    }
}

impl Mode for RainbowCycle {
    fn name(&self) -> &str {
        "RainbowCycle"
    }

    fn update(&mut self, strip: &mut Strip) {
        for (i, &color) in self.colors.iter().enumerate() {
            strip.set_led_rgb(i, color);
        }
        strip.send();

        // Rotate colors: move last to front
        if let Some(last) = self.colors.pop() {
            self.colors.insert(0, last);
        }
    }

    fn get_opts(&self) -> OptMap {
        OptMap::new()
    }

    fn set_opts(&mut self, _opts: OptMap) {}
}

/// KITT-style scanning light animation.
///
/// A single bright LED sweeps back and forth with optional
/// trailing effect and fade.
pub struct NightRider {
    opts: OptMap,
    position: isize,
    direction: isize,
}

impl NightRider {
    /// Creates a new NightRider mode with default options.
    pub fn new() -> Self {
        let mut opts = OptMap::new();
        opts.insert("color".to_string(), OptValue::color(colors::PURPLE));
        opts.insert("tail_color".to_string(), OptValue::color(colors::BLUE));
        opts.insert("fade".to_string(), OptValue::bool(true));
        opts.insert("fill_color".to_string(), OptValue::color(colors::BLACK));

        Self {
            opts,
            position: 0,
            direction: 1,
        }
    }

    /// Gets a color option by key, returning black if invalid.
    fn color_opt(&self, key: &str) -> [u8; 3] {
        self.opts
            .get(key)
            .and_then(|v| OptValue::parse_color(&v.value).ok())
            .unwrap_or(colors::BLACK)
    }

    /// Gets a boolean option by key, returning false if invalid.
    fn bool_opt(&self, key: &str) -> bool {
        self.opts.get(key).and_then(|v| v.value.as_bool()).unwrap_or(false)
    }
}

impl Default for NightRider {
    fn default() -> Self {
        Self::new()
    }
}

impl Mode for NightRider {
    fn name(&self) -> &str {
        "NightRider"
    }

    fn update(&mut self, strip: &mut Strip) {
        let fill_color = self.color_opt("fill_color");
        let fade = self.bool_opt("fade");
        let color = self.color_opt("color");
        let tail_color = self.color_opt("tail_color");

        // Clear or fade background
        if fade {
            for i in 0..strip.num_leds {
                if let Some(rgb) = strip.get_led(i) {
                    strip.set_led_rgb(i, strip.scale_color(rgb, 0.60));
                }
            }
        } else {
            for i in 0..strip.num_leds {
                strip.set_led_rgb(i, fill_color);
            }
        }

        // Draw main LED and tail
        let pos = self.position as usize;
        if pos < strip.num_leds {
            strip.set_led_rgb(pos, color);

            // Draw tail if configured
            if tail_color != colors::BLACK {
                let tail_pos = (self.position - self.direction) as usize;
                let at_edge = self.position == 0 || self.position == (strip.num_leds as isize - 1);

                if !at_edge && tail_pos < strip.num_leds {
                    strip.set_led_rgb(tail_pos, tail_color);
                }
            }
        }

        // Update position with bounce
        self.position += self.direction;
        if self.position >= strip.num_leds as isize {
            self.position = strip.num_leds as isize - 1;
            self.direction = -1;
        } else if self.position < 0 {
            self.position = 1;
            self.direction = 1;
        }

        strip.send();
    }

    fn get_opts(&self) -> OptMap {
        self.opts.clone()
    }

    fn set_opts(&mut self, opts: OptMap) {
        for (key, val) in opts {
            if self.opts.contains_key(&key) {
                self.opts.insert(key, val);
            }
        }
    }
}

/// Two-particle collision animation with explosion effect.
///
/// Two colored dots move toward each other, collide, and produce
/// a decaying red explosion at the collision point.
pub struct Collider {
    opts: OptMap,
    pos_a: isize,
    dir_a: isize,
    pos_b: isize,
    dir_b: isize,
    collision_leds: Vec<usize>,
    collision_strength: i32,
}

impl Collider {
    /// Creates a new collider mode for the given strip length.
    pub fn new(strip: &Strip) -> Self {
        let mut opts = OptMap::new();
        opts.insert("color_a".to_string(), OptValue::color(colors::PURPLE));
        opts.insert("color_b".to_string(), OptValue::color(colors::BLUE));
        opts.insert("collision_decay".to_string(), OptValue::int(7));

        Self {
            opts,
            pos_a: 0,
            dir_a: 1,
            pos_b: strip.num_leds as isize,
            dir_b: -1,
            collision_leds: Vec::new(),
            collision_strength: 100,
        }
    }

    fn color_opt(&self, key: &str) -> [u8; 3] {
        self.opts
            .get(key)
            .and_then(|v| OptValue::parse_color(&v.value).ok())
            .unwrap_or(colors::BLACK)
    }

    fn int_opt(&self, key: &str) -> i64 {
        self.opts.get(key).and_then(|v| v.value.as_i64()).unwrap_or(0)
    }

    /// Updates a single particle position with boundary bounce.
    fn update_particle(&self, pos: isize, dir: isize, strip: &Strip) -> (isize, isize) {
        let new_pos = pos + dir;
        let max = strip.num_leds as isize - 1;

        if new_pos > max {
            (max, -1)
        } else if new_pos < 0 {
            (0, 1)
        } else {
            (new_pos, dir)
        }
    }
}

impl Mode for Collider {
    fn name(&self) -> &str {
        "Collider"
    }

    fn update(&mut self, strip: &mut Strip) {
        strip.fill(colors::BLACK);

        // Draw collision effect if active
        if !self.collision_leds.is_empty() {
            let intensity = self.collision_strength as f64 / 100.0;
            let color = strip.scale_color(colors::RED, intensity);

            for &led in &self.collision_leds {
                strip.set_led_rgb(led, color);
            }

            let decay = self.int_opt("collision_decay") as i32;
            self.collision_strength -= decay;

            if self.collision_strength <= 0 {
                self.collision_leds.clear();
                self.collision_strength = 100;
            }
        }

        // Update and draw particles
        let color_a = self.color_opt("color_a");
        let color_b = self.color_opt("color_b");

        (self.pos_a, self.dir_a) = self.update_particle(self.pos_a, self.dir_a, strip);
        (self.pos_b, self.dir_b) = self.update_particle(self.pos_b, self.dir_b, strip);

        if self.pos_a < strip.num_leds as isize {
            strip.set_led_rgb(self.pos_a as usize, color_a);
        }
        if self.pos_b >= 0 && (self.pos_b as usize) < strip.num_leds {
            strip.set_led_rgb(self.pos_b as usize, color_b);
        }

        // Check for collision
        if self.pos_a >= self.pos_b {
            self.dir_a = -self.dir_a;
            self.dir_b = -self.dir_b;
            self.collision_leds = vec![self.pos_a as usize, self.pos_b as usize];
        }

        strip.send();
    }

    fn get_opts(&self) -> OptMap {
        self.opts.clone()
    }

    fn set_opts(&mut self, opts: OptMap) {
        for (key, val) in opts {
            if self.opts.contains_key(&key) {
                self.opts.insert(key, val);
            }
        }
    }
}

/// Alternating color pattern animation.
///
/// Cycles through a fixed set of colors across the LED strip.
/// Used for holiday themes like Christmas, Mardi Gras, and RGB.
pub struct PatternMode {
    colors: Vec<[u8; 3]>,
    offset: usize,
    name: &'static str,
}

impl PatternMode {
    /// Creates a Christmas-themed pattern (red/green).
    pub fn christmas() -> Self {
        Self {
            colors: colors::CHRISTMAS_COLORS.to_vec(),
            offset: 0,
            name: "Christmas",
        }
    }

    /// Creates a Mardi Gras-themed pattern (purple/green/yellow).
    pub fn mardi_gras() -> Self {
        Self {
            colors: colors::MARDI_GRAS_COLORS.to_vec(),
            offset: 0,
            name: "MardiGras",
        }
    }

    /// Creates an RGB cycle pattern.
    pub fn rgb() -> Self {
        Self {
            colors: colors::RGB_COLORS.to_vec(),
            offset: 0,
            name: "ArrGeeBee",
        }
    }
}

impl Mode for PatternMode {
    fn name(&self) -> &str {
        self.name
    }

    fn update(&mut self, strip: &mut Strip) {
        // Shift pattern on even cycles to create animation
        if (self.colors.len() + strip.num_leds) % 2 == 0 {
            self.offset = (self.offset + 1) % self.colors.len();
        }

        let mut color_idx = self.offset;
        for i in 0..strip.num_leds {
            strip.set_led_rgb(i, self.colors[color_idx]);
            color_idx = (color_idx + 1) % self.colors.len();
        }

        strip.send();
    }

    fn get_opts(&self) -> OptMap {
        OptMap::new()
    }

    fn set_opts(&mut self, _opts: OptMap) {}
}

/// Random sparkle effect with decay.
///
/// Random LEDs light up with random colors and fade over time,
/// creating a twinkling star effect.
pub struct Sparkle {
    opts: OptMap,
    rng: SmallRng,
    color_buf: [u8; 3],
}

impl Sparkle {
    /// Creates a new sparkle mode with default options.
    pub fn new() -> Self {
        let mut opts = OptMap::new();
        opts.insert("low".to_string(), OptValue::int(0));
        opts.insert("high".to_string(), OptValue::int(255));
        opts.insert("r_on".to_string(), OptValue::bool(true));
        opts.insert("g_on".to_string(), OptValue::bool(false));
        opts.insert("b_on".to_string(), OptValue::bool(true));
        opts.insert("decay".to_string(), OptValue::float(0.92));

        Self {
            opts,
            rng: SmallRng::from_entropy(),
            color_buf: [0, 0, 0],
        }
    }

    fn int_opt(&self, key: &str) -> i64 {
        self.opts.get(key).and_then(|v| v.value.as_i64()).unwrap_or(0)
    }

    fn bool_opt(&self, key: &str) -> bool {
        self.opts.get(key).and_then(|v| v.value.as_bool()).unwrap_or(false)
    }

    fn float_opt(&self, key: &str) -> f64 {
        self.opts.get(key).and_then(|v| v.value.as_f64()).unwrap_or(0.0)
    }

    /// Generates a random RGB color based on current options.
    fn random_color(&mut self) -> [u8; 3] {
        let low = self.int_opt("low") as u8;
        let high = self.int_opt("high") as u8;
        let range = (high - low).max(1);

        let r_on = self.bool_opt("r_on");
        let g_on = self.bool_opt("g_on");
        let b_on = self.bool_opt("b_on");

        [
            if r_on { low + self.rng.gen_range(0..range) } else { 0 },
            if g_on { low + self.rng.gen_range(0..range) } else { 0 },
            if b_on { low + self.rng.gen_range(0..range) } else { 0 },
        ]
    }
}

impl Default for Sparkle {
    fn default() -> Self {
        Self::new()
    }
}

impl Mode for Sparkle {
    fn name(&self) -> &str {
        "Sparkle"
    }

    fn update(&mut self, strip: &mut Strip) {
        let decay = self.float_opt("decay");
        let threshold = strip.num_leds * 3;

        for i in 0..strip.num_leds {
            // Random chance to sparkle
            if self.rng.gen_range(0..threshold) == 0 {
                self.color_buf = self.random_color();
                strip.set_led_rgb(i, self.color_buf);
            } else if let Some(rgb) = strip.get_led(i) {
                // Apply decay
                strip.set_led_rgb(
                    i,
                    [
                        (rgb[0] as f64 * decay) as u8,
                        (rgb[1] as f64 * decay) as u8,
                        (rgb[2] as f64 * decay) as u8,
                    ],
                );
            }
        }

        strip.send();
    }

    fn get_opts(&self) -> OptMap {
        self.opts.clone()
    }

    fn set_opts(&mut self, opts: OptMap) {
        for (key, val) in opts {
            if self.opts.contains_key(&key) {
                self.opts.insert(key, val);
            }
        }
    }
}

/// Pulsing brightness animation.
///
/// Creates a breathing effect by cycling brightness while
/// maintaining a fixed color.
pub struct Breathe {
    opts: OptMap,
    brightness: i64,
    direction: i64,
}

impl Breathe {
    /// Creates a new breathe mode with default options.
    pub fn new() -> Self {
        let mut opts = OptMap::new();
        opts.insert("r".to_string(), OptValue::int(255));
        opts.insert("g".to_string(), OptValue::int(0));
        opts.insert("b".to_string(), OptValue::int(255));
        opts.insert("low".to_string(), OptValue::int(0));
        opts.insert("high".to_string(), OptValue::int(255));

        Self {
            opts,
            brightness: 0,
            direction: 1,
        }
    }

    fn int_opt(&self, key: &str) -> i64 {
        self.opts.get(key).and_then(|v| v.value.as_i64()).unwrap_or(0)
    }
}

impl Default for Breathe {
    fn default() -> Self {
        Self::new()
    }
}

impl Mode for Breathe {
    fn name(&self) -> &str {
        "Breathe"
    }

    fn update(&mut self, strip: &mut Strip) {
        let r = self.int_opt("r") as u8;
        let g = self.int_opt("g") as u8;
        let b = self.int_opt("b") as u8;
        let low = self.int_opt("low");
        let high = self.int_opt("high");

        strip.set_brightness(self.brightness as u8);
        strip.fill([r, g, b]);

        // Update brightness with bounce
        if self.brightness >= high {
            self.direction = -1;
        } else if self.brightness <= low {
            self.direction = 1;
        }
        self.brightness += self.direction;
    }

    fn get_opts(&self) -> OptMap {
        self.opts.clone()
    }

    fn set_opts(&mut self, opts: OptMap) {
        for (key, val) in opts {
            if self.opts.contains_key(&key) {
                self.opts.insert(key, val);
            }
        }
    }
}

/// Static single color mode.
///
/// Displays a single solid color across all LEDs.
pub struct Solid {
    opts: OptMap,
}

impl Solid {
    /// Creates a new solid mode with default yellow color.
    pub fn new() -> Self {
        let mut opts = OptMap::new();
        opts.insert("color".to_string(), OptValue::color(colors::YELLOW));

        Self { opts }
    }

    fn color_opt(&self, key: &str) -> [u8; 3] {
        self.opts
            .get(key)
            .and_then(|v| OptValue::parse_color(&v.value).ok())
            .unwrap_or(colors::BLACK)
    }
}

impl Default for Solid {
    fn default() -> Self {
        Self::new()
    }
}

impl Mode for Solid {
    fn name(&self) -> &str {
        "Solid"
    }

    fn update(&mut self, strip: &mut Strip) {
        let color = self.color_opt("color");
        strip.fill(color);
    }

    fn get_opts(&self) -> OptMap {
        self.opts.clone()
    }

    fn set_opts(&mut self, opts: OptMap) {
        for (key, val) in opts {
            if self.opts.contains_key(&key) {
                self.opts.insert(key, val);
            }
        }
    }

    fn on_load(&mut self, set_delay: &dyn Fn(f64)) {
        set_delay(0.250);
    }
}

/// Fixed color modes (White, Off).
///
/// Simple modes that display a constant color with no options.
pub struct FixedColor {
    color: [u8; 3],
    name: &'static str,
    delay: f64,
}

impl FixedColor {
    /// Creates a white mode.
    pub fn white() -> Self {
        Self {
            color: colors::WHITE,
            name: "White",
            delay: 0.250,
        }
    }

    /// Creates an off/black mode.
    pub fn off() -> Self {
        Self {
            color: colors::BLACK,
            name: "Off",
            delay: 0.250,
        }
    }
}

impl Mode for FixedColor {
    fn name(&self) -> &str {
        self.name
    }

    fn update(&mut self, strip: &mut Strip) {
        strip.fill(self.color);
    }

    fn get_opts(&self) -> OptMap {
        OptMap::new()
    }

    fn set_opts(&mut self, _opts: OptMap) {}

    fn on_load(&mut self, set_delay: &dyn Fn(f64)) {
        set_delay(self.delay);
    }
}
