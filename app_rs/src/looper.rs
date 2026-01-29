//! Core animation loop and state management.
//!
//! The looper manages the background thread that drives LED animations,
//! coordinating timing across multiple strips and handling state transitions.

use std::cell::Cell;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use tracing::{debug, error, info, warn};

use crate::error::{Error, Result};
use crate::modes::{available_modes, create_mode, Mode};
use crate::opts::OptMap;
use crate::strip::Strip;

/// Default animation delay between frames in seconds.
pub const DEFAULT_DELAY: f64 = 0.025;

/// Delay for static modes (Solid, White, Off) in seconds.
pub const STATIC_DELAY: f64 = 0.250;

/// State for a single LED strip including its animation mode.
pub struct StripState {
    /// The LED strip hardware interface.
    pub strip: Strip,

    /// Current animation mode.
    pub mode: Box<dyn Mode>,

    /// Delay between animation frames (seconds).
    pub delay: f64,

    /// Timestamp of last animation update.
    last_update: Instant,
}

impl StripState {
    /// Creates a new strip state with the specified strip and initial mode.
    pub fn new(strip: Strip, mode_name: &str) -> Result<Self> {
        let mode = create_mode(mode_name, &strip);

        Ok(Self {
            strip,
            mode,
            delay: DEFAULT_DELAY,
            last_update: Instant::now(),
        })
    }

    /// Returns the current brightness level.
    pub fn brightness(&self) -> u8 {
        self.strip.brightness()
    }

    /// Sets the brightness level for this strip.
    pub fn set_brightness(&mut self, val: u8) {
        self.strip.set_brightness(val);
    }

    /// Returns the current mode name.
    pub fn mode_name(&self) -> String {
        self.mode.name().to_string()
    }

    /// Returns a copy of the current options.
    pub fn options(&self) -> OptMap {
        self.mode.get_opts()
    }

    /// Applies options to the current mode.
    pub fn set_options(&mut self, opts: OptMap) {
        self.mode.set_opts(opts);
    }

    /// Checks if this strip needs an update based on its delay.
    fn needs_update(&self) -> bool {
        self.last_update.elapsed() >= Duration::from_secs_f64(self.delay)
    }

    /// Updates the animation and returns the time until next update.
    fn update(&mut self) -> Duration {
        self.mode.update(&mut self.strip);
        self.last_update = Instant::now();
        Duration::from_secs_f64(self.delay)
    }

    /// Returns time remaining until next update.
    fn time_until_update(&self) -> Duration {
        let delay = Duration::from_secs_f64(self.delay);
        delay.saturating_sub(self.last_update.elapsed())
    }
}

/// Serializable metadata for a strip (for API responses).
#[derive(Debug, Clone, Serialize)]
pub struct StripInfo {
    /// Device identifier.
    pub id: u8,

    /// Hostname or IP address.
    pub hostname: String,

    /// UDP port number.
    pub port: u16,

    /// Number of LEDs on the strip.
    pub num_leds: usize,

    /// Current animation mode name.
    pub mode: String,

    /// Current brightness level (0-255).
    pub brightness: u8,

    /// Current delay between frames (seconds).
    pub delay: f64,
}

/// Global state managed by the looper.
pub struct LooperState {
    /// Individual states for each connected strip.
    strip_states: Vec<StripState>,

    /// Whether debug mode is enabled.
    debug_mode: bool,

    /// Current loop state: "running" or "paused".
    loop_state: LoopState,

    /// Remaining iterations in debug mode (-1 = unlimited).
    iterations_remaining: i64,

    /// Condition variable for pause/resume control.
    condvar: Arc<(Mutex<bool>, Condvar)>,
}

/// Enumeration of possible loop states.
#[derive(Debug, Clone, Copy, PartialEq)]
enum LoopState {
    Running,
    Paused,
}

impl std::fmt::Display for LoopState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "running"),
            Self::Paused => write!(f, "paused"),
        }
    }
}

impl LooperState {
    /// Creates a new looper state with default configuration.
    pub fn new(debug_mode: bool) -> Result<Self> {
        let strips = Self::create_default_strips()?;

        let strip_states = strips
            .into_iter()
            .map(|strip| StripState::new(strip, "NightRider"))
            .collect::<Result<Vec<_>>>()?;

        info!("Initialized {} LED strips", strip_states.len());

        Ok(Self {
            strip_states,
            debug_mode,
            loop_state: LoopState::Running,
            iterations_remaining: -1,
            condvar: Arc::new((Mutex::new(true), Condvar::new())),
        })
    }

    /// Creates default strip configurations.
    fn create_default_strips() -> Result<Vec<Strip>> {
        Ok(vec![
            Strip::new(1, "esp32c6-00.lan", 67)?,
            Strip::new(2, "esp32c6-01.lan", 83)?,
        ])
    }

    /// Finds the index of a strip by its device ID.
    fn find_strip_index(&self, strip_id: u8) -> Option<usize> {
        self.strip_states.iter().position(|s| s.strip.dev_id == strip_id)
    }

    // ==================== Global Operations ====================

    /// Returns the name of the first strip's mode (for backward compatibility).
    pub fn current_mode(&self) -> String {
        self.strip_states
            .first()
            .map(|s| s.mode_name())
            .unwrap_or_else(|| "Unknown".to_string())
    }

    /// Sets the mode for all strips.
    pub fn set_mode(&mut self, mode_name: &str) -> Result<()> {
        if !available_modes().contains(&mode_name.to_string()) {
            return Err(Error::UnknownMode(mode_name.to_string()));
        }

        let is_static = matches!(mode_name, "Solid" | "White" | "Off");

        for state in &mut self.strip_states {
            let mut mode = create_mode(mode_name, &state.strip);

            // Allow mode to configure preferred delay via callback
            let preferred_delay: Cell<Option<f64>> = Cell::new(None);
            mode.on_load(&|delay: f64| {
                preferred_delay.set(Some(delay));
            });

            state.mode = mode;
            state.delay = preferred_delay
                .get()
                .unwrap_or_else(|| if is_static { STATIC_DELAY } else { DEFAULT_DELAY });
        }

        info!("Set global mode to {}", mode_name);
        Ok(())
    }

    /// Returns the brightness of the first strip.
    pub fn brightness(&self) -> u8 {
        self.strip_states.first().map(|s| s.brightness()).unwrap_or(255)
    }

    /// Sets brightness for all strips.
    pub fn set_brightness(&mut self, val: u8) {
        for state in &mut self.strip_states {
            state.set_brightness(val);
        }
        debug!("Set global brightness to {}", val);
    }

    /// Returns the delay of the first strip.
    pub fn delay(&self) -> f64 {
        self.strip_states.first().map(|s| s.delay).unwrap_or(DEFAULT_DELAY)
    }

    /// Sets delay for all strips.
    pub fn set_delay(&mut self, val: f64) {
        for state in &mut self.strip_states {
            state.delay = val;
        }
        debug!("Set global delay to {}", val);
    }

    /// Returns options from the first strip.
    pub fn options(&self) -> OptMap {
        self.strip_states.first().map(|s| s.options()).unwrap_or_default()
    }

    /// Sets options for all strips.
    pub fn set_options(&mut self, opts: OptMap) {
        for state in &mut self.strip_states {
            state.set_options(opts.clone());
        }
    }

    // ==================== Per-Strip Operations ====================

    /// Gets the mode name for a specific strip.
    pub fn strip_mode(&self, strip_id: u8) -> Option<String> {
        self.find_strip_index(strip_id).map(|idx| self.strip_states[idx].mode_name())
    }

    /// Sets the mode for a specific strip.
    pub fn set_strip_mode(&mut self, strip_id: u8, mode_name: &str) -> Result<()> {
        if !available_modes().contains(&mode_name.to_string()) {
            return Err(Error::UnknownMode(mode_name.to_string()));
        }

        let idx = self.find_strip_index(strip_id).ok_or(Error::StripNotFound(strip_id))?;
        let state = &mut self.strip_states[idx];

        let mut mode = create_mode(mode_name, &state.strip);
        mode.on_load(&|_delay: f64| {});
        state.mode = mode;

        let is_static = matches!(mode_name, "Solid" | "White" | "Off");
        state.delay = if is_static { STATIC_DELAY } else { DEFAULT_DELAY };

        debug!("Set strip {} mode to {}", strip_id, mode_name);
        Ok(())
    }

    /// Gets brightness for a specific strip.
    pub fn strip_brightness(&self, strip_id: u8) -> Option<u8> {
        self.find_strip_index(strip_id).map(|idx| self.strip_states[idx].brightness())
    }

    /// Sets brightness for a specific strip.
    pub fn set_strip_brightness(&mut self, strip_id: u8, val: u8) -> Result<u8> {
        let idx = self.find_strip_index(strip_id).ok_or(Error::StripNotFound(strip_id))?;
        self.strip_states[idx].set_brightness(val);
        Ok(self.strip_states[idx].brightness())
    }

    /// Gets delay for a specific strip.
    pub fn strip_delay(&self, strip_id: u8) -> Option<f64> {
        self.find_strip_index(strip_id).map(|idx| self.strip_states[idx].delay)
    }

    /// Sets delay for a specific strip.
    pub fn set_strip_delay(&mut self, strip_id: u8, val: f64) -> Result<f64> {
        let idx = self.find_strip_index(strip_id).ok_or(Error::StripNotFound(strip_id))?;
        self.strip_states[idx].delay = val;
        Ok(self.strip_states[idx].delay)
    }

    /// Gets options for a specific strip.
    pub fn strip_options(&self, strip_id: u8) -> Option<OptMap> {
        self.find_strip_index(strip_id).map(|idx| self.strip_states[idx].options())
    }

    /// Sets options for a specific strip.
    pub fn set_strip_options(&mut self, strip_id: u8, opts: OptMap) -> Result<OptMap> {
        let idx = self.find_strip_index(strip_id).ok_or(Error::StripNotFound(strip_id))?;
        self.strip_states[idx].set_options(opts);
        Ok(self.strip_states[idx].options())
    }

    /// Returns information about all strips.
    pub fn strips_info(&self) -> Vec<StripInfo> {
        self.strip_states
            .iter()
            .map(|s| StripInfo {
                id: s.strip.dev_id,
                hostname: s.strip.udp_ip.clone(),
                port: s.strip.udp_port,
                num_leds: s.strip.num_leds,
                mode: s.mode_name(),
                brightness: s.brightness(),
                delay: s.delay,
            })
            .collect()
    }

    /// Configures a strip's network settings (debug mode only).
    pub fn configure_strip(&mut self, strip_id: u8, hostname: Option<String>, port: Option<u16>) -> Result<StripInfo> {
        if !self.debug_mode {
            return Err(Error::DebugModeRequired);
        }

        let idx = self.find_strip_index(strip_id).ok_or(Error::StripNotFound(strip_id))?;
        let state = &mut self.strip_states[idx];

        if let Some(h) = hostname {
            state.strip.udp_ip = h;
        }
        if let Some(p) = port {
            state.strip.udp_port = p;
        }

        info!(
            "Configured strip {}: {}:{}",
            strip_id, state.strip.udp_ip, state.strip.udp_port
        );

        Ok(StripInfo {
            id: state.strip.dev_id,
            hostname: state.strip.udp_ip.clone(),
            port: state.strip.udp_port,
            num_leds: state.strip.num_leds,
            mode: state.mode_name(),
            brightness: state.brightness(),
            delay: state.delay,
        })
    }

    /// Controls the animation loop (debug mode only).
    pub fn control_loop(&mut self, iterations: Option<i64>, next_state: Option<String>) -> Result<LoopControlResponse> {
        if !self.debug_mode {
            return Err(Error::DebugModeRequired);
        }

        match (iterations, next_state) {
            (Some(iters), _) => {
                // Run N iterations then pause
                self.iterations_remaining = iters;
                self.loop_state = LoopState::Running;
                let (lock, cvar) = &*self.condvar;
                *lock.lock().map_err(|_| Error::LockPoisoned)? = true;
                cvar.notify_all();
                info!("Loop set to run for {} iterations", iters);
            }
            (None, Some(state)) => match state.as_str() {
                "pause" => {
                    self.loop_state = LoopState::Paused;
                    let (lock, _) = &*self.condvar;
                    *lock.lock().map_err(|_| Error::LockPoisoned)? = false;
                    info!("Loop paused");
                }
                "running" => {
                    self.loop_state = LoopState::Running;
                    self.iterations_remaining = -1;
                    let (lock, cvar) = &*self.condvar;
                    *lock.lock().map_err(|_| Error::LockPoisoned)? = true;
                    cvar.notify_all();
                    info!("Loop resumed");
                }
                _ => warn!("Unknown loop state: {}", state),
            },
            _ => {}
        }

        Ok(LoopControlResponse {
            state: self.loop_state.to_string(),
            iterations_remaining: self.iterations_remaining,
        })
    }

    /// Returns the current loop state.
    pub fn loop_state(&self) -> LoopStateResponse {
        LoopStateResponse {
            state: self.loop_state.to_string(),
            iterations_remaining: self.iterations_remaining,
            debug_mode: self.debug_mode,
        }
    }
}

/// Response from a loop control operation.
#[derive(Debug, Clone, Serialize)]
pub struct LoopControlResponse {
    /// Current state: "running" or "paused".
    pub state: String,

    /// Remaining iterations (-1 = unlimited).
    pub iterations_remaining: i64,
}

/// Current loop state information.
#[derive(Debug, Clone, Serialize)]
pub struct LoopStateResponse {
    /// Current state: "running" or "paused".
    pub state: String,

    /// Remaining iterations.
    pub iterations_remaining: i64,

    /// Whether debug mode is enabled.
    pub debug_mode: bool,
}

/// Main controller that manages the animation loop thread.
pub struct Looper {
    /// Shared state accessible from both threads.
    state: Arc<Mutex<LooperState>>,
}

impl Looper {
    /// Creates a new looper and starts the animation thread.
    pub fn new(debug_mode: bool) -> Result<Self> {
        let state = Arc::new(Mutex::new(LooperState::new(debug_mode)?));
        let state_clone = Arc::clone(&state);

        thread::spawn(move || {
            if let Err(e) = Self::run_loop(state_clone) {
                error!("Animation loop crashed: {}", e);
            }
        });

        info!("Looper initialized with debug_mode={}", debug_mode);
        Ok(Self { state })
    }

    /// Main animation loop that runs in a background thread.
    fn run_loop(state: Arc<Mutex<LooperState>>) -> Result<()> {
        loop {
            // Check for pause state (debug mode only)
            Self::handle_pause(&state)?;

            // Calculate minimum wait time across all strips
            let min_wait = Self::update_strips(&state)?;

            // Handle iteration counting in debug mode
            Self::handle_iterations(&state)?;

            // Sleep until next update needed
            if min_wait > Duration::ZERO {
                thread::sleep(min_wait);
            }
        }
    }

    /// Handles pause/resume logic for debug mode.
    fn handle_pause(state: &Arc<Mutex<LooperState>>) -> Result<()> {
        let guard = state.lock().map_err(|_| Error::LockPoisoned)?;

        if guard.debug_mode && guard.loop_state == LoopState::Paused {
            let condvar = Arc::clone(&guard.condvar);
            drop(guard);

            let (lock, cvar) = &*condvar;
            let mut running = lock.lock().map_err(|_| Error::LockPoisoned)?;

            while !*running {
                running = cvar.wait(running).map_err(|_| Error::LockPoisoned)?;
            }
        }

        Ok(())
    }

    /// Updates all strips and returns minimum wait time.
    fn update_strips(state: &Arc<Mutex<LooperState>>) -> Result<Duration> {
        let mut guard = state.lock().map_err(|_| Error::LockPoisoned)?;
        let mut min_wait = Duration::from_secs(1); // Default max wait

        for strip_state in &mut guard.strip_states {
            if strip_state.needs_update() {
                strip_state.update();
            }

            let wait = strip_state.time_until_update();
            if wait < min_wait {
                min_wait = wait;
            }
        }

        Ok(min_wait)
    }

    /// Handles iteration counting in debug mode.
    fn handle_iterations(state: &Arc<Mutex<LooperState>>) -> Result<()> {
        let mut guard = state.lock().map_err(|_| Error::LockPoisoned)?;

        if guard.debug_mode && guard.iterations_remaining > 0 {
            guard.iterations_remaining -= 1;

            if guard.iterations_remaining == 0 {
                guard.loop_state = LoopState::Paused;
                let (lock, _) = &*guard.condvar;
                *lock.lock().map_err(|_| Error::LockPoisoned)? = false;
                info!("Iteration count reached zero, pausing");
            }
        }

        Ok(())
    }

    /// Returns a clone of the shared state for external access.
    pub fn state(&self) -> Arc<Mutex<LooperState>> {
        Arc::clone(&self.state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loop_state_display() {
        assert_eq!(LoopState::Running.to_string(), "running");
        assert_eq!(LoopState::Paused.to_string(), "paused");
    }

    #[test]
    fn test_strip_info_creation() {
        // Note: This test would require mocking the Strip
        // For now, just test the struct creation
        let info = StripInfo {
            id: 1,
            hostname: "test.lan".to_string(),
            port: 4210,
            num_leds: 50,
            mode: "Solid".to_string(),
            brightness: 128,
            delay: 0.05,
        };

        assert_eq!(info.id, 1);
        assert_eq!(info.hostname, "test.lan");
    }
}
