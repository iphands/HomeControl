use std::sync::Arc;
use tokio::runtime::Runtime;

mod app;
mod packet;
mod udp_listener;

use app::LEDStripEmulator;

fn main() {
    // Initialize Tokio runtime
    let rt = Runtime::new().expect("Failed to create Tokio runtime");
    let rt = Arc::new(rt);

    // Create application
    let mut app = LEDStripEmulator::new(rt.clone());

    // Initialize with default strip configurations
    if let Err(e) = app.initialize() {
        eprintln!("Failed to initialize app: {}", e);
        return;
    }

    // Start UDP listener in background
    app.start_udp_listener();

    // Set up egui/wgpu renderer options
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("LED Strip Emulator"),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    // Run application
    let _ = eframe::run_native(
        "LED Strip Emulator",
        options,
        Box::new(|_cc| {
            Ok(Box::new(app))
        }),
    );
}