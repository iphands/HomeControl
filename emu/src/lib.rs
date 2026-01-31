mod api_client;
mod types;
mod gui;

pub use api_client::ApiClient;
pub use types::*;
pub use gui::LedEmulatorApp;

/// Main function for the emulator
pub fn run_emulator() {
    println!("HomeCtrl Emulator initialized");
    
    // Start the GUI application
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("HomeCtrl LED Emulator"),
        ..Default::default()
    };
    
    let _ = eframe::run_native(
        "HomeCtrl LED Emulator",
        native_options,
        Box::new(|_cc| Ok(Box::new(LedEmulatorApp::new(ApiClient::new("http://localhost:5000"))))),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_config_serialization() {
        let config = StripConfig {
            id: 1,
            hostname: "localhost".to_string(),
            port: 5000,
            num_leds: 60,
            mode: "RainbowCycle".to_string(),
            brightness: 128,
            delay: 0.025,
        };
        assert_eq!(config.id, 1);
        assert_eq!(config.num_leds, 60);
    }
}
