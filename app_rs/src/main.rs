//! HomeCtrl LED Strip Controller
//!
//! A high-performance LED strip controller with HTTP API and animation modes.
//!
//! # Features
//! - Multiple animation modes (Rainbow, NightRider, Sparkle, etc.)
//! - Per-strip and global control via REST API
//! - Debug mode with step-through animation control
//! - UDP communication with ESP32 LED controllers
//!
//! # Usage
//! ```bash
//! # Run in normal mode
//! cargo run
//!
//! # Run in debug mode (enables looper control)
//! cargo run -- --debug
//! ```

mod colors;
mod error;
mod looper;
mod modes;
mod opts;
mod server;
mod strip;

use std::env;

use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use crate::error::Result;
use crate::looper::Looper;

/// Application entry point.
///
/// Initializes logging, parses command line arguments, creates the looper,
/// and starts the HTTP server.
#[actix_web::main]
async fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder().with_max_level(Level::INFO).finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");

    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    let debug_mode = args.contains(&"--debug".to_string()) || args.contains(&"-d".to_string());

    info!("╔══════════════════════════════════════╗");
    info!("║     HomeCtrl LED Controller          ║");
    info!("║     Rust Edition v0.1.0              ║");
    info!("╚══════════════════════════════════════╝");
    info!("Debug mode: {}", debug_mode);

    // Create the looper (starts animation thread)
    let looper = Looper::new(debug_mode)?;

    // Start HTTP server (blocks until shutdown)
    server::start_server(looper)
        .await
        .map_err(|e| error::Error::Network(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_structure() {
        // Verify all modules are accessible
        let _ = colors::RED;
        let _ = error::Error::LockPoisoned;
    }
}
