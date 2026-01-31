// Simple LED Strip Emulator for testing compatibility

use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

use crate::packet::LEDPacket;
use crate::udp_listener::{create_default_strips, StripConfig, StripMessage, UDPListener};

/// State for a single LED strip
#[derive(Debug)]
pub struct StripState {
    pub config: StripConfig,
    pub led_colors: Vec<(u8, u8, u8)>, // RGB colors
    pub current_sequence: u8,
    pub current_brightness: u8,
    pub packet_count: usize,
    pub last_update: Option<std::time::Instant>,
}

impl StripState {
    pub fn new(config: StripConfig) -> Self {
        let num_leds = config.num_leds as usize;
        Self {
            config,
            led_colors: vec![(0, 0, 0); num_leds],
            current_sequence: 0,
            current_brightness: 255,
            packet_count: 0,
            last_update: None,
        }
    }

    pub fn update_from_packet(&mut self, packet: &LEDPacket) {
        self.current_sequence = packet.sequence;
        self.current_brightness = packet.brightness;
        self.packet_count += 1;
        self.last_update = Some(std::time::Instant::now());

        // Update LED colors from packet
        let new_colors = packet.get_led_data_with_brightness();
        let new_colors = new_colors
            .into_iter()
            .chain(std::iter::repeat((0, 0, 0)))
            .take(self.config.num_leds as usize)
            .collect();

        self.led_colors = new_colors;
    }
}

/// Main application state
pub struct LEDStripEmulator {
    rt: Arc<Runtime>,
    strips: HashMap<u8, StripState>, // device_id -> strip_state
    udp_listener: Option<UDPListener>,
    message_receiver: Option<mpsc::UnboundedReceiver<StripMessage>>,
    show_debug: bool,
    auto_update: bool,
}

impl LEDStripEmulator {
    pub fn new(rt: Arc<Runtime>) -> Self {
        Self {
            rt,
            strips: HashMap::new(),
            udp_listener: None,
            message_receiver: None,
            show_debug: true,
            auto_update: true,
        }
    }

    pub fn initialize(&mut self) -> anyhow::Result<()> {
        let strip_configs = create_default_strips();

        // Create strip states
        self.strips.clear();
        for config in strip_configs {
            self.strips.insert(config.id, StripState::new(config));
        }

        println!("LED Strip Emulator initialized");
        for (device_id, state) in &self.strips {
            println!(
                "  Strip {}: {} LEDs on port {}",
                device_id, state.config.num_leds, state.config.port
            );
        }

        Ok(())
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        // Set up UDP communication
        let (tx, rx) = mpsc::unbounded_channel();

        let strip_configs = self
            .strips
            .values()
            .map(|state| state.config.clone())
            .collect();

        let listener = UDPListener::new(strip_configs, tx)?;
        self.udp_listener = Some(listener);
        self.message_receiver = Some(rx);

        println!("LED Strip Emulator running with UDP listener");
        println!("Press Ctrl+C to stop...");

        // Simple event loop
        loop {
            self.process_messages();

            // Check for Ctrl+C every 100ms
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    /// Start UDP listener in background
    pub fn start_udp_listener(&self) {
        if let Some(listener) = &self.udp_listener {
            let listener = listener.clone();
            let rt = self.rt.clone();
            rt.spawn(async move {
                if let Err(e) = listener.start().await {
                    eprintln!("UDP listener error: {}", e);
                }
            });
        }
    }

    /// Process incoming messages from UDP listener
    pub fn process_messages(&mut self) {
        if let Some(receiver) = &mut self.message_receiver {
            while let Ok(message) = receiver.try_recv() {
                match message {
                    StripMessage::PacketReceived(packet) => {
                        if let Some(strip_state) = self.strips.get_mut(&packet.device_id) {
                            strip_state.update_from_packet(&packet);
                        }
                    }
                    StripMessage::ListenerError(error) => {
                        eprintln!("Listener error: {}", error);
                    }
                }
            }
        }
    }
}

impl eframe::App for LEDStripEmulator {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process UDP messages
        self.process_messages();

        // Top panel with controls
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("LED Strip Emulator");
                ui.separator();
                ui.checkbox(&mut self.show_debug, "Show Debug Info");
                ui.checkbox(&mut self.auto_update, "Auto Update");
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });

        // Main content area with strip visualizations
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("LED Strips");
            
            for (device_id, strip_state) in &self.strips {
                ui.group(|ui| {
                    ui.label(format!("Strip {} ({} LEDs)", device_id, strip_state.config.num_leds));
                    
                    // Simple LED visualization
                    let led_count = strip_state.led_colors.len();
                    if led_count > 0 {
                        ui.separator();
                        // Show first few LEDs as colored rectangles
                        let display_count = std::cmp::min(led_count, 50);
                        for chunk in strip_state.led_colors[..display_count].chunks(10) {
                            ui.horizontal(|ui| {
                                for (r, g, b) in chunk {
                                    ui.colored_label(
                                        egui::Color32::from_rgb(*r, *g, *b),
                                        "██"
                                    );
                                }
                            });
                        }
                        
                        if led_count > 50 {
                            ui.label(format!("... and {} more", led_count - 50));
                        }
                    }
                    
                    if self.show_debug {
                        ui.separator();
                        ui.label(format!("Brightness: {}", strip_state.current_brightness));
                        ui.label(format!("Sequence: {}", strip_state.current_sequence));
                        ui.label(format!("Packets: {}", strip_state.packet_count));
                        if let Some(last) = strip_state.last_update {
                            let elapsed = last.elapsed().as_secs();
                            ui.label(format!("Last update: {}s ago", elapsed));
                        }
                    }
                });
            }
        });
    }
}
