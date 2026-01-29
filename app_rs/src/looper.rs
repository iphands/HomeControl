use crate::modes::{create_mode, Mode};
use crate::opts::OptValue;
use crate::strip::Strip;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_DELAY: f64 = 0.025;

pub struct StripState {
    pub strip: Strip,
    pub mode: Box<dyn Mode + Send>,
    pub delay: f64,
    pub last_update: Instant,
}

impl StripState {
    pub fn new(strip: Strip, mode_name: &str) -> Self {
        let mode = create_mode(mode_name, &strip);
        Self {
            strip,
            mode,
            delay: DEFAULT_DELAY,
            last_update: Instant::now(),
        }
    }

    pub fn get_brightness(&self) -> u8 {
        self.strip.get_brightness()
    }

    pub fn set_brightness(&mut self, val: u8) {
        self.strip.set_brightness(val);
    }

    pub fn get_mode_name(&self) -> String {
        self.mode.name().to_string()
    }

    pub fn get_opts(&self) -> HashMap<String, OptValue> {
        self.mode.get_opts()
    }

    pub fn set_opts(&mut self, opts: HashMap<String, OptValue>) {
        self.mode.set_opts(opts);
    }
}

pub struct LooperState {
    pub strip_states: Vec<StripState>,
    pub debug_mode: bool,
    pub loop_state: String,        // "running" or "paused"
    pub iterations_remaining: i64, // -1 means unlimited
    pub condvar: Arc<(Mutex<bool>, Condvar)>,
}

impl LooperState {
    pub fn new(debug_mode: bool) -> Self {
        let strips = vec![
            Strip::new(1, "esp32c6-00.lan", 67),
            Strip::new(2, "esp32c6-01.lan", 83),
        ];

        let strip_states: Vec<StripState> = strips
            .into_iter()
            .map(|strip| StripState::new(strip, "NightRider"))
            .collect();

        Self {
            strip_states,
            debug_mode,
            loop_state: "running".to_string(),
            iterations_remaining: -1,
            condvar: Arc::new((Mutex::new(true), Condvar::new())),
        }
    }

    // --- Global functions (apply to ALL strips for backward compatibility) ---

    pub fn get_current_mode_name(&self) -> String {
        if let Some(state) = self.strip_states.first() {
            state.get_mode_name()
        } else {
            "Unknown".to_string()
        }
    }

    pub fn set_mode(&mut self, mode_name: &str) {
        for strip_state in &mut self.strip_states {
            let mut mode = create_mode(mode_name, &strip_state.strip);
            mode.load_cb(&|_d: f64| {
                // Delay callback - handled below
            });
            strip_state.mode = mode;
        }

        // Apply delay changes from load_cb
        match mode_name {
            "Solid" | "White" | "Off" => {
                for strip_state in &mut self.strip_states {
                    strip_state.delay = 0.250;
                }
            }
            _ => {
                for strip_state in &mut self.strip_states {
                    strip_state.delay = DEFAULT_DELAY;
                }
            }
        }
    }

    pub fn get_brightness(&self) -> u8 {
        self.strip_states.first().map(|s| s.get_brightness()).unwrap_or(255)
    }

    pub fn set_brightness(&mut self, val: u8) {
        for strip_state in &mut self.strip_states {
            strip_state.set_brightness(val);
        }
    }

    pub fn get_delay(&self) -> f64 {
        self.strip_states.first().map(|s| s.delay).unwrap_or(DEFAULT_DELAY)
    }

    pub fn set_delay(&mut self, val: f64) {
        for strip_state in &mut self.strip_states {
            strip_state.delay = val;
        }
    }

    pub fn get_opts(&self) -> HashMap<String, OptValue> {
        if let Some(state) = self.strip_states.first() {
            state.get_opts()
        } else {
            HashMap::new()
        }
    }

    pub fn set_opts(&mut self, opts: HashMap<String, OptValue>) {
        for strip_state in &mut self.strip_states {
            strip_state.set_opts(opts.clone());
        }
    }

    // --- Per-strip functions ---

    fn find_strip_state(&self, strip_id: u8) -> Option<usize> {
        self.strip_states.iter().position(|s| s.strip.dev_id == strip_id)
    }

    pub fn get_strip_mode(&self, strip_id: u8) -> Option<String> {
        self.find_strip_state(strip_id)
            .map(|idx| self.strip_states[idx].get_mode_name())
    }

    pub fn set_strip_mode(&mut self, strip_id: u8, mode_name: &str) -> Option<String> {
        if let Some(idx) = self.find_strip_state(strip_id) {
            let strip_state = &mut self.strip_states[idx];
            let mut mode = create_mode(mode_name, &strip_state.strip);

            // Apply load_cb for delay
            match mode_name {
                "Solid" | "White" | "Off" => {
                    strip_state.delay = 0.250;
                }
                _ => {
                    strip_state.delay = DEFAULT_DELAY;
                }
            }

            mode.load_cb(&|_d: f64| {});
            strip_state.mode = mode;
            Some(strip_state.get_mode_name())
        } else {
            None
        }
    }

    pub fn get_strip_brightness(&self, strip_id: u8) -> Option<u8> {
        self.find_strip_state(strip_id)
            .map(|idx| self.strip_states[idx].get_brightness())
    }

    pub fn set_strip_brightness(&mut self, strip_id: u8, val: u8) -> Option<u8> {
        if let Some(idx) = self.find_strip_state(strip_id) {
            self.strip_states[idx].set_brightness(val);
            Some(self.strip_states[idx].get_brightness())
        } else {
            None
        }
    }

    pub fn get_strip_delay(&self, strip_id: u8) -> Option<f64> {
        self.find_strip_state(strip_id)
            .map(|idx| self.strip_states[idx].delay)
    }

    pub fn set_strip_delay(&mut self, strip_id: u8, val: f64) -> Option<f64> {
        if let Some(idx) = self.find_strip_state(strip_id) {
            self.strip_states[idx].delay = val;
            Some(self.strip_states[idx].delay)
        } else {
            None
        }
    }

    pub fn get_strip_opts(&self, strip_id: u8) -> Option<HashMap<String, OptValue>> {
        self.find_strip_state(strip_id)
            .map(|idx| self.strip_states[idx].get_opts())
    }

    pub fn set_strip_opts(&mut self, strip_id: u8, opts: HashMap<String, OptValue>) -> Option<HashMap<String, OptValue>> {
        if let Some(idx) = self.find_strip_state(strip_id) {
            self.strip_states[idx].set_opts(opts);
            Some(self.strip_states[idx].get_opts())
        } else {
            None
        }
    }

    pub fn get_strips(&self) -> Vec<StripInfo> {
        self.strip_states
            .iter()
            .map(|s| StripInfo {
                id: s.strip.dev_id,
                hostname: s.strip.udp_ip.clone(),
                port: s.strip.udp_port,
                num_leds: s.strip.num_leds,
                mode: s.get_mode_name(),
                brightness: s.get_brightness(),
                delay: s.delay,
            })
            .collect()
    }

    pub fn configure_strip(&mut self, strip_id: u8, hostname: Option<String>, port: Option<u16>) -> Result<StripInfo, String> {
        if !self.debug_mode {
            return Err("debug mode not enabled".to_string());
        }

        if let Some(idx) = self.find_strip_state(strip_id) {
            let strip_state = &mut self.strip_states[idx];
            if let Some(h) = hostname {
                strip_state.strip.udp_ip = h;
            }
            if let Some(p) = port {
                strip_state.strip.udp_port = p;
            }
            Ok(StripInfo {
                id: strip_state.strip.dev_id,
                hostname: strip_state.strip.udp_ip.clone(),
                port: strip_state.strip.udp_port,
                num_leds: strip_state.strip.num_leds,
                mode: strip_state.get_mode_name(),
                brightness: strip_state.get_brightness(),
                delay: strip_state.delay,
            })
        } else {
            Err(format!("strip {} not found", strip_id))
        }
    }

    pub fn loop_control(&mut self, iterations: Option<i64>, next_state: Option<String>) -> LoopControlResult {
        if !self.debug_mode {
            return LoopControlResult {
                state: self.loop_state.clone(),
                iterations_remaining: self.iterations_remaining,
                error: Some("debug mode not enabled".to_string()),
            };
        }

        if let Some(iters) = iterations {
            // Run N iterations then pause
            self.iterations_remaining = iters;
            self.loop_state = "running".to_string();
            let (lock, cvar) = &*self.condvar;
            let mut running = lock.lock().unwrap();
            *running = true;
            cvar.notify_all();
        } else if let Some(state) = next_state {
            match state.as_str() {
                "pause" => {
                    self.loop_state = "paused".to_string();
                    let (lock, _) = &*self.condvar;
                    let mut running = lock.lock().unwrap();
                    *running = false;
                }
                "running" => {
                    self.loop_state = "running".to_string();
                    self.iterations_remaining = -1;
                    let (lock, cvar) = &*self.condvar;
                    let mut running = lock.lock().unwrap();
                    *running = true;
                    cvar.notify_all();
                }
                _ => {}
            }
        }

        LoopControlResult {
            state: self.loop_state.clone(),
            iterations_remaining: self.iterations_remaining,
            error: None,
        }
    }

    pub fn get_loop_state(&self) -> LoopStateResponse {
        LoopStateResponse {
            state: self.loop_state.clone(),
            iterations_remaining: self.iterations_remaining,
            debug_mode: self.debug_mode,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StripInfo {
    pub id: u8,
    pub hostname: String,
    pub port: u16,
    pub num_leds: usize,
    pub mode: String,
    pub brightness: u8,
    pub delay: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoopControlResult {
    pub state: String,
    pub iterations_remaining: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoopStateResponse {
    pub state: String,
    pub iterations_remaining: i64,
    pub debug_mode: bool,
}

pub struct Looper {
    state: Arc<Mutex<LooperState>>,
}

impl Looper {
    pub fn new(debug_mode: bool) -> Self {
        let state = Arc::new(Mutex::new(LooperState::new(debug_mode)));

        // Clone state for the loop thread
        let state_clone = Arc::clone(&state);

        thread::spawn(move || {
            Self::loop_thread(state_clone);
        });

        Self { state }
    }

    fn loop_thread(state: Arc<Mutex<LooperState>>) {
        loop {
            // Check if we should wait (debug mode + paused)
            {
                let state_guard = state.lock().unwrap();
                if state_guard.debug_mode && state_guard.loop_state == "paused" {
                    // Wait on condition variable
                    let condvar = Arc::clone(&state_guard.condvar);
                    drop(state_guard);

                    let (lock, cvar) = &*condvar;
                    let mut running = lock.lock().unwrap();
                    while !*running {
                        running = cvar.wait(running).unwrap();
                    }
                }
            }

            let mut min_wait = Duration::from_secs(1); // Default max wait

            // Update modes and send - per-strip timing
            {
                let mut state_guard = state.lock().unwrap();

                for strip_state in &mut state_guard.strip_states {
                    let elapsed = strip_state.last_update.elapsed();
                    let delay_duration = Duration::from_secs_f64(strip_state.delay);

                    if elapsed >= delay_duration {
                        strip_state.mode.update(&mut strip_state.strip);
                        strip_state.last_update = Instant::now();
                    }

                    let remaining = delay_duration.saturating_sub(strip_state.last_update.elapsed());
                    if remaining < min_wait {
                        min_wait = remaining;
                    }
                }

                // Handle iteration counting in debug mode
                if state_guard.debug_mode && state_guard.iterations_remaining > 0 {
                    state_guard.iterations_remaining -= 1;
                    if state_guard.iterations_remaining == 0 {
                        state_guard.loop_state = "paused".to_string();
                        let (lock, _) = &*state_guard.condvar;
                        let mut running = lock.lock().unwrap();
                        *running = false;
                    }
                }
            }

            // Sleep until next strip needs update
            if min_wait > Duration::ZERO {
                thread::sleep(min_wait);
            }
        }
    }

    pub fn get_state(&self) -> Arc<Mutex<LooperState>> {
        Arc::clone(&self.state)
    }
}
