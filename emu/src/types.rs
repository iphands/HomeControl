use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct StripConfig {
    pub id: i32,
    pub hostname: String,
    pub port: u16,
    pub num_leds: usize,
    pub mode: String,
    pub brightness: u8,
    pub delay: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StripData {
    pub strips: Vec<StripConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModeResponse {
    pub modes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CurrentModeResponse {
    pub mode: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BrightnessResponse {
    pub brightness: u8,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DelayResponse {
    pub delay: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoopState {
    pub state: String,
    pub iterations_remaining: i32,
    pub debug_mode: bool,
}
