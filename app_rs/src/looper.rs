use crate::modes::{create_mode, Mode};
use crate::opts::OptValue;
use crate::strip::Strip;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Condvar};
use std::thread;
use std::time::{Duration, Instant};

pub struct LooperState {
    pub strips: Vec<Strip>,
    pub modes: Vec<Box<dyn Mode + Send>>,
    pub delay: f64,
    pub debug_mode: bool,
    pub loop_state: String,  // "running" or "paused"
    pub iterations_remaining: i64,  // -1 means unlimited
    pub condvar: Arc<(Mutex<bool>, Condvar)>,
}

impl LooperState {
    pub fn new(debug_mode: bool) -> Self {
        let strips = vec![
            Strip::new(1, "esp32c6-00.lan", 67),
            Strip::new(2, "esp32c6-01.lan", 83),
        ];
        
        let mut modes: Vec<Box<dyn Mode + Send>> = Vec::new();
        for strip in &strips {
            modes.push(create_mode("NightRider", strip));
        }
        
        Self {
            strips,
            modes,
            delay: 0.025,
            debug_mode,
            loop_state: "running".to_string(),
            iterations_remaining: -1,
            condvar: Arc::new((Mutex::new(true), Condvar::new())),
        }
    }
    
    pub fn get_current_mode_name(&self) -> String {
        if let Some(mode) = self.modes.first() {
            mode.name().to_string()
        } else {
            "Unknown".to_string()
        }
    }
    
    pub fn set_mode(&mut self, mode_name: &str) {
        let mut new_modes: Vec<Box<dyn Mode + Send>> = Vec::new();
        for strip in &self.strips {
            let mut mode = create_mode(mode_name, strip);
            mode.load_cb(&|_d: f64| { 
                // We can't modify self.delay from here directly
                // Store the request and handle it after
            });
            new_modes.push(mode);
        }
        self.modes = new_modes;
        
        // Apply delay changes from load_cb - since load_cb doesn't actually do anything
        // in this simplified version, we'll just set known delays for specific modes
        match mode_name {
            "Solid" | "White" | "Off" => self.delay = 0.250,
            _ => self.delay = 0.025,
        }
    }
    
    pub fn get_brightness(&self) -> u8 {
        self.strips.first().map(|s| s.get_brightness()).unwrap_or(255)
    }
    
    pub fn set_brightness(&mut self, val: u8) {
        for strip in &mut self.strips {
            strip.set_brightness(val);
        }
    }
    
    pub fn get_opts(&self) -> HashMap<String, OptValue> {
        if let Some(mode) = self.modes.first() {
            mode.get_opts()
        } else {
            HashMap::new()
        }
    }
    
    pub fn set_opts(&mut self, opts: HashMap<String, OptValue>) {
        for mode in &mut self.modes {
            mode.set_opts(opts.clone());
        }
    }
    
    pub fn get_strips(&self) -> Vec<StripInfo> {
        self.strips.iter().map(|s| StripInfo {
            id: s.dev_id,
            hostname: s.udp_ip.clone(),
            port: s.udp_port,
            num_leds: s.num_leds,
        }).collect()
    }
    
    pub fn configure_strip(&mut self, strip_id: u8, hostname: Option<String>, port: Option<u16>) -> Result<StripInfo, String> {
        if !self.debug_mode {
            return Err("debug mode not enabled".to_string());
        }
        
        for strip in &mut self.strips {
            if strip.dev_id == strip_id {
                if let Some(h) = hostname {
                    strip.udp_ip = h;
                }
                if let Some(p) = port {
                    strip.udp_port = p;
                }
                return Ok(StripInfo {
                    id: strip.dev_id,
                    hostname: strip.udp_ip.clone(),
                    port: strip.udp_port,
                    num_leds: strip.num_leds,
                });
            }
        }
        
        Err(format!("strip {} not found", strip_id))
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
            let start = Instant::now();
            
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
            
            // Update modes and send
            {
                let mut state_guard = state.lock().unwrap();
                let len = state_guard.strips.len().min(state_guard.modes.len());
                
                for i in 0..len {
                    // We need to update each mode with its corresponding strip
                    // Use unsafe to get mutable references to different elements
                    let strip = &mut state_guard.strips[i] as *mut Strip;
                    let mode = &mut state_guard.modes[i];
                    unsafe {
                        mode.update(&mut *strip);
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
            
            // Calculate sleep time
            let elapsed = start.elapsed();
            let delay = {
                let state_guard = state.lock().unwrap();
                Duration::from_secs_f64(state_guard.delay)
            };
            
            if elapsed < delay {
                thread::sleep(delay - elapsed);
            }
        }
    }
    
    pub fn get_state(&self) -> Arc<Mutex<LooperState>> {
        Arc::clone(&self.state)
    }
}
