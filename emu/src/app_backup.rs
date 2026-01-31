use anyhow::Result;
use eframe::egui;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::runtime::Runtime;

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

/// Stop the server via HTTP API call
async fn stop_server() -> Result<(), anyhow::Error> {
    let client = reqwest::Client::new();
    
    // Try to call the server's stop endpoint if it exists
    // For now, we'll just attempt to pause the looper
    match client
        .post("http://localhost:5000/api/looper")
        .json(&serde_json::json!({
            "next_state": "pause"
        }))
        .send()
        .await
    {
        Ok(_) => println!("Sent pause signal to server"),
        Err(e) => println!("Failed to pause server: {}", e),
    }

    Ok(())
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

    /// Initialize emulator with default strip configurations
    pub fn initialize(&mut self) -> Result<()> {
        let strip_configs = create_default_strips();
        
        // Create strip states
        self.strips.clear();
        for config in strip_configs {
            self.strips.insert(config.id, StripState::new(config));
        }

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

        println!("LED Strip Emulator initialized");
        for (device_id, state) in &self.strips {
            println!(
                "  Strip {}: {} LEDs on port {}",
                device_id, state.config.num_leds, state.config.port
            );
        }

        Ok(())
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
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
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
                    if ui.button("Stop All").clicked() {
                        // Try to stop server and exit
                        tokio::spawn(async move {
                            if let Err(e) = stop_server().await {
                                eprintln!("Failed to stop server: {}", e);
                            }
                        });
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });

        // Main content area with strip visualizations
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut device_ids: Vec<u8> = self.strips.keys().cloned().collect();
            device_ids.sort();

            for device_id in device_ids {
                if let Some(strip_state) = self.strips.get(&device_id) {
                    self.show_strip_panel(ui, strip_state, device_id);
                }
            }
        });

        // Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(receiver) = &mut self.message_receiver {
                    let _ = receiver.try_recv(); // This ensures the channel stays active
                }
                
                ui.label("Listening for UDP packets...");
                ui.separator();
                
                let total_packets: usize = self.strips.values().map(|s| s.packet_count).sum();
                ui.label(format!("Total packets: {}", total_packets));
            });
        });

        // Request repaint for smooth updates
        if self.auto_update {
            ctx.request_repaint();
        }
    }

    fn show_strip_panel(&self, ui: &mut egui::Ui, strip_state: &StripState, device_id: u8) {
        let strip_id = format!("Strip {} ({} LEDs)", device_id, strip_state.config.num_leds);
        
        egui::CollapsingHeader::new(&strip_id)
            .default_open(true)
            .show(ui, |ui| {
                // Status information
                if self.show_debug {
                    ui.horizontal(|ui| {
                        ui.label(format!("Sequence: {}", strip_state.current_sequence));
                        ui.label(format!("Brightness: {}", strip_state.current_brightness));
                        ui.label(format!("Packets: {}", strip_state.packet_count));
                        
                        if let Some(last_update) = strip_state.last_update {
                            let elapsed = last_update.elapsed().as_secs();
                            ui.label(format!("Last update: {}s ago", elapsed));
                        } else {
                            ui.label("Last update: never");
                        }
                    });
                    ui.separator();
                }

                // LED visualization
                self.show_led_strip(ui, strip_state);
            });
    }

    fn show_led_strip(ui: &mut egui::Ui, strip_state: &StripState) {
        let available_width = ui.available_width();
        let led_size = 8.0;
        let led_spacing = 2.0;
        let leds_per_row = ((available_width + led_spacing) / (led_size + led_spacing)) as usize;
        
        let led_colors = strip_state.led_colors.clone(); // Clone to avoid borrow issues
        let num_leds = led_colors.len();
        let rows = (num_leds + leds_per_row - 1) / leds_per_row;

        // Calculate the rectangle for the LED strip area
        let total_width = leds_per_row.min(num_leds) as f32 * (led_size + led_spacing) - led_spacing;
        let total_height = rows as f32 * (led_size + led_spacing) - led_spacing;

        let response = ui.allocate_response(
            egui::Vec2::new(total_width, total_height),
            egui::Sense::hover(),
        );
        
        let painter = ui.painter();
        let rect = response.rect;

        // Draw LEDs
        for (i, &(r, g, b)) in led_colors.iter().enumerate() {
            let row = i / leds_per_row;
            let col = i % leds_per_row;
            
            let x = rect.min.x + col as f32 * (led_size + led_spacing);
            let y = rect.min.y + row as f32 * (led_size + led_spacing);
            
            let led_rect = egui::Rect::from_min_size(
                egui::Pos2::new(x, y),
                egui::Vec2::new(led_size, led_size),
            );

            let color = egui::Color32::from_rgb(r, g, b);
            painter.rect_filled(led_rect, 2.0, color);
            
            // Add subtle border
            painter.rect_stroke(
                led_rect,
                2.0,
                egui::Stroke::new(1.0, egui::Color32::from_gray(80)),
            );
        }

        // Show hover information
        if response.hovered() {
            let lit_count = led_colors.iter().filter(|(r, g, b)| *r > 0 || *g > 0 || *b > 0).count();
            response.on_hover_text(format!(
                "{} / {} LEDs lit",
                lit_count,
                led_colors.len()
            ));
        }
    }
}