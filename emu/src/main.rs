//! LED Strip Emulator for HomeCtrl
//!
//! This emulator receives UDP packets from the LED controller server
//! and renders the LED strips in a GPU-accelerated GUI window.

use clap::Parser;
use eframe::egui;
use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::thread;

/// UDP Protocol constants
const MSG_TYPE_LED_STRIP: u8 = 1;
const HEADER_SIZE: usize = 5;

/// Header byte offsets
const TYPE_OFFSET: usize = 0;
const SEQ_OFFSET: usize = 1;
const BRIGHTNESS_OFFSET: usize = 2;
const LED_COUNT_OFFSET: usize = 3;
const DEVICE_ID_OFFSET: usize = 4;
const LED_DATA_OFFSET: usize = 5;

/// Command line arguments
#[derive(Parser, Debug)]
#[command(name = "led-emu")]
#[command(about = "LED Strip Emulator for HomeCtrl")]
struct Args {
    /// UDP port to listen on
    #[arg(short, long, default_value_t = 4210)]
    port: u16,

    /// Bind address
    #[arg(short, long, default_value = "0.0.0.0")]
    bind: String,

    /// LED size in pixels
    #[arg(long, default_value_t = 12)]
    led_size: u32,

    /// Gap between LEDs in pixels
    #[arg(long, default_value_t = 2)]
    led_gap: u32,
}

/// RGB color for a single LED
#[derive(Clone, Copy, Debug, Default)]
struct LedColor {
    r: u8,
    g: u8,
    b: u8,
}

impl LedColor {
    fn to_egui_color(&self, brightness: u8) -> egui::Color32 {
        let factor = brightness as f32 / 255.0;
        egui::Color32::from_rgb(
            (self.r as f32 * factor) as u8,
            (self.g as f32 * factor) as u8,
            (self.b as f32 * factor) as u8,
        )
    }
}

/// State for a single LED strip
#[derive(Clone, Debug)]
struct StripState {
    device_id: u8,
    led_count: u8,
    brightness: u8,
    sequence: u8,
    leds: Vec<LedColor>,
    last_update: std::time::Instant,
}

impl StripState {
    fn new(device_id: u8) -> Self {
        Self {
            device_id,
            led_count: 0,
            brightness: 255,
            sequence: 0,
            leds: Vec::new(),
            last_update: std::time::Instant::now(),
        }
    }

    fn update_from_packet(&mut self, packet: &[u8]) {
        if packet.len() < HEADER_SIZE {
            return;
        }

        self.sequence = packet[SEQ_OFFSET];
        self.brightness = packet[BRIGHTNESS_OFFSET];
        self.led_count = packet[LED_COUNT_OFFSET];

        let expected_len = HEADER_SIZE + (self.led_count as usize * 3);
        if packet.len() < expected_len {
            return;
        }

        // Resize LED array if needed
        if self.leds.len() != self.led_count as usize {
            self.leds
                .resize(self.led_count as usize, LedColor::default());
        }

        // Extract LED colors
        for i in 0..self.led_count as usize {
            let offset = LED_DATA_OFFSET + (i * 3);
            self.leds[i] = LedColor {
                r: packet[offset],
                g: packet[offset + 1],
                b: packet[offset + 2],
            };
        }

        self.last_update = std::time::Instant::now();
    }
}

/// Shared state between UDP listener and GUI
#[derive(Default)]
struct SharedState {
    strips: HashMap<u8, StripState>,
    packets_received: u64,
    should_exit: bool,
}

/// Start the UDP listener thread
fn start_udp_listener(bind_addr: String, port: u16, state: Arc<Mutex<SharedState>>) {
    thread::spawn(move || {
        let addr = format!("{}:{}", bind_addr, port);
        let socket = match UdpSocket::bind(&addr) {
            Ok(s) => {
                tracing::info!("UDP listener bound to {}", addr);
                s
            }
            Err(e) => {
                tracing::error!("Failed to bind UDP socket to {}: {}", addr, e);
                return;
            }
        };

        // Set non-blocking so we can check exit flag
        socket
            .set_read_timeout(Some(std::time::Duration::from_millis(100)))
            .ok();

        let mut buf = [0u8; 512];

        loop {
            // Check if we should exit
            {
                let state = state.lock().unwrap();
                if state.should_exit {
                    tracing::info!("UDP listener shutting down");
                    return;
                }
            }

            match socket.recv_from(&mut buf) {
                Ok((len, _src)) => {
                    if len < HEADER_SIZE {
                        continue;
                    }

                    let msg_type = buf[TYPE_OFFSET];
                    if msg_type != MSG_TYPE_LED_STRIP {
                        continue;
                    }

                    let device_id = buf[DEVICE_ID_OFFSET];

                    let mut state = state.lock().unwrap();
                    state.packets_received += 1;

                    let strip = state
                        .strips
                        .entry(device_id)
                        .or_insert_with(|| StripState::new(device_id));

                    strip.update_from_packet(&buf[..len]);
                }
                Err(e) => {
                    if e.kind() != std::io::ErrorKind::WouldBlock
                        && e.kind() != std::io::ErrorKind::TimedOut
                    {
                        tracing::warn!("UDP recv error: {}", e);
                    }
                }
            }
        }
    });
}

/// Main application state
struct EmulatorApp {
    state: Arc<Mutex<SharedState>>,
    led_size: f32,
    led_gap: f32,
    port: u16,
}

impl EmulatorApp {
    fn new(state: Arc<Mutex<SharedState>>, args: &Args) -> Self {
        Self {
            state,
            led_size: args.led_size as f32,
            led_gap: args.led_gap as f32,
            port: args.port,
        }
    }
}

impl eframe::App for EmulatorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Request continuous repaints for smooth animation
        ctx.request_repaint();

        let state = self.state.lock().unwrap();
        let strips: Vec<_> = state.strips.values().cloned().collect();
        let packets = state.packets_received;
        drop(state);

        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("LED Strip Emulator");
                ui.separator();
                ui.label(format!("Port: {}", self.port));
                ui.separator();
                ui.label(format!("Packets: {}", packets));
                ui.separator();
                ui.label(format!("Strips: {}", strips.len()));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Exit").clicked() {
                        let mut state = self.state.lock().unwrap();
                        state.should_exit = true;
                        std::process::exit(0);
                    }
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if strips.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label("Waiting for LED data...\n\nStart the server and LED animations should appear here.");
                });
            } else {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for strip in &strips {
                        self.render_strip(ui, strip);
                        ui.add_space(10.0);
                    }
                });
            }
        });
    }
}

impl EmulatorApp {
    fn render_strip(&self, ui: &mut egui::Ui, strip: &StripState) {
        let age = strip.last_update.elapsed();
        let age_str = if age.as_secs() > 0 {
            format!("{}s ago", age.as_secs())
        } else {
            format!("{}ms ago", age.as_millis())
        };

        ui.horizontal(|ui| {
            ui.label(format!(
                "Strip {} | {} LEDs | Brightness: {} | Seq: {} | Updated: {}",
                strip.device_id, strip.led_count, strip.brightness, strip.sequence, age_str
            ));
        });

        // Calculate layout
        let available_width = ui.available_width();
        let led_total_width = self.led_size + self.led_gap;
        let leds_per_row = ((available_width / led_total_width) as usize).max(1);

        let (response, painter) = ui.allocate_painter(
            egui::vec2(
                available_width,
                ((strip.leds.len() + leds_per_row - 1) / leds_per_row) as f32 * led_total_width
                    + self.led_size,
            ),
            egui::Sense::hover(),
        );

        let rect = response.rect;
        let start_pos = rect.min;

        // Draw dark background
        painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(20, 20, 25));

        // Draw each LED
        for (i, led) in strip.leds.iter().enumerate() {
            let row = i / leds_per_row;
            let col = i % leds_per_row;

            let x = start_pos.x + self.led_gap + col as f32 * led_total_width;
            let y = start_pos.y + self.led_gap + row as f32 * led_total_width;

            let led_rect = egui::Rect::from_min_size(
                egui::pos2(x, y),
                egui::vec2(self.led_size, self.led_size),
            );

            let color = led.to_egui_color(strip.brightness);

            // Draw LED glow effect (larger, semi-transparent circle behind)
            let glow_color =
                egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 80);
            painter.circle_filled(led_rect.center(), self.led_size * 0.8, glow_color);

            // Draw LED
            painter.rect_filled(led_rect, self.led_size / 4.0, color);

            // Draw LED border
            painter.rect_stroke(
                led_rect,
                self.led_size / 4.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 65)),
                egui::StrokeKind::Inside,
            );
        }

        ui.add_space(4.0);
    }
}

fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    tracing::info!("Starting LED Emulator on {}:{}", args.bind, args.port);

    // Create shared state
    let state = Arc::new(Mutex::new(SharedState::default()));

    // Start UDP listener
    start_udp_listener(args.bind.clone(), args.port, Arc::clone(&state));

    // Calculate initial window size
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 400.0])
            .with_title("LED Strip Emulator"),
        ..Default::default()
    };

    // Run the GUI
    eframe::run_native(
        "LED Emulator",
        native_options,
        Box::new(|_cc| Ok(Box::new(EmulatorApp::new(Arc::clone(&state), &args)))),
    )
    .expect("Failed to run eframe");
}
