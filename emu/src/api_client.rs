use reqwest;

use crate::types::{StripData, ModeResponse, CurrentModeResponse, BrightnessResponse, DelayResponse, LoopState};

/// API client for interacting with the HomeCtrl server
#[derive(Clone)]
pub struct ApiClient {
    client: reqwest::Client,
    base_url: String,
}

impl ApiClient {
    /// Creates a new API client
    pub fn new(base_url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.to_string(),
        }
    }

    /// Fetches all strip configurations from the server
    pub async fn get_strips(&self) -> Result<StripData, reqwest::Error> {
        let response = self.client
            .get(&format!("{}/api/strips", self.base_url))
            .send()
            .await?;
        
        response.json().await
    }

    /// Gets the current mode for a specific strip
    pub async fn get_strip_mode(&self, strip_id: i32) -> Result<CurrentModeResponse, reqwest::Error> {
        let response = self.client
            .get(&format!("{}/api/strips/{}/mode", self.base_url, strip_id))
            .send()
            .await?;
        
        response.json().await
    }

    /// Gets the current brightness for a specific strip
    pub async fn get_strip_brightness(&self, strip_id: i32) -> Result<BrightnessResponse, reqwest::Error> {
        let response = self.client
            .get(&format!("{}/api/strips/{}/brightness", self.base_url, strip_id))
            .send()
            .await?;
        
        response.json().await
    }

    /// Gets the current delay for a specific strip
    pub async fn get_strip_delay(&self, strip_id: i32) -> Result<DelayResponse, reqwest::Error> {
        let response = self.client
            .get(&format!("{}/api/strips/{}/delay", self.base_url, strip_id))
            .send()
            .await?;
        
        response.json().await
    }

    /// Gets the current loop state
    pub async fn get_loop_state(&self) -> Result<LoopState, reqwest::Error> {
        let response = self.client
            .get(&format!("{}/api/looper", self.base_url))
            .send()
            .await?;
        
        response.json().await
    }

    /// Gets available modes
    pub async fn get_modes(&self) -> Result<ModeResponse, reqwest::Error> {
        let response = self.client
            .get(&format!("{}/api/modes", self.base_url))
            .send()
            .await?;
        
        response.json().await
    }

    /// Gets current global mode
    pub async fn get_current_mode(&self) -> Result<CurrentModeResponse, reqwest::Error> {
        let response = self.client
            .get(&format!("{}/api/modes/current", self.base_url))
            .send()
            .await?;
        
        response.json().await
    }

    /// Gets current global brightness
    pub async fn get_brightness(&self) -> Result<BrightnessResponse, reqwest::Error> {
        let response = self.client
            .get(&format!("{}/api/brightness", self.base_url))
            .send()
            .await?;
        
        response.json().await
    }

    /// Gets current global delay
    pub async fn get_delay(&self) -> Result<DelayResponse, reqwest::Error> {
        let response = self.client
            .get(&format!("{}/api/delay", self.base_url))
            .send()
            .await?;
        
        response.json().await
    }
}