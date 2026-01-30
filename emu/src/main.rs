use macroquad::prelude::*;
use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

#[derive(Clone, Debug)]
struct LEDStrip {
    _strip_id: u8,
    _num_leds: usize,
    colors: Vec<(u8, u8, u8)>,
    brightness: u8,
    sequence: u8,
    packet_count: u64,
}

impl LEDStrip {
    fn new(strip_id: u8, num_leds: usize) -> Self {
        Self {
            _strip_id: strip_id,
            _num_leds: num_leds,
            colors: vec![(0, 0, 0); num_leds],
            brightness: 255,
            sequence: 0,
            packet_count: 0,
        }
    }

    fn update_from_packet(&mut self, packet: &LEDPacket) {
        self.brightness = packet.brightness;
        self.sequence = packet.sequence;
        self.colors = packet.led_data.clone();
        self.packet_count += 1;
    }
}

#[derive(Clone, Debug)]
struct LEDPacket {
    _msg_type: u8,
    sequence: u8,
    brightness: u8,
    _num_leds: u8,
    device_id: u8,
    led_data: Vec<(u8, u8, u8)>,
}

impl LEDPacket {
    fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 5 {
            return None;
        }

        let msg_type = data[0];
        let sequence = data[1];
        let brightness = data[2];
        let num_leds = data[3];
        let device_id = data[4];

        let mut led_data = Vec::with_capacity(num_leds as usize);
        for i in 0..num_leds as usize {
            let offset = 5 + (i * 3);
            if offset + 2 < data.len() {
                let r = data[offset];
                let g = data[offset + 1];
                let b = data[offset + 2];
                led_data.push((r, g, b));
            } else {
                led_data.push((0, 0, 0));
            }
        }

        Some(Self {
            _msg_type: msg_type,
            sequence,
            brightness,
            _num_leds: num_leds,
            device_id,
            led_data,
        })
    }
}

/// UDP Listener that receives LED packets
struct UDPListener {
    port: u16,
    tx: mpsc::Sender<LEDPacket>,
}

impl UDPListener {
    fn new(port: u16, tx: mpsc::Sender<LEDPacket>) -> Self {
        Self { port, tx }
    }

    fn run(self) {
        let addr = format!("0.0.0.0:{}", self.port);
        let socket = match UdpSocket::bind(&addr) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to bind UDP socket on port {}: {}", self.port, e);
                return;
            }
        };

        // Set socket timeout to allow checking for shutdown
        socket
            .set_read_timeout(Some(std::time::Duration::from_millis(100)))
            .ok();

        println!("UDP Listener started on port {}", self.port);

        let mut buf = vec![0u8; 2048];
        loop {
            match socket.recv_from(&mut buf) {
                Ok((len, _addr)) => {
                    if let Some(packet) = LEDPacket::from_bytes(&buf[..len]) {
                        if self.tx.send(packet).is_err() {
                            // Channel closed, exit
                            break;
                        }
                    }
                }
                Err(e) => {
                    // Check if it's a timeout (expected) or actual error
                    if e.kind() != std::io::ErrorKind::WouldBlock
                        && e.kind() != std::io::ErrorKind::TimedOut
                    {
                        eprintln!("UDP receive error: {}", e);
                    }
                }
            }
        }
    }
}

/// Shared state between async task and main thread
struct SharedState {
    strips: HashMap<u8, LEDStrip>,
}

impl SharedState {
    fn new(strips_config: &[(u8, usize)]) -> Self {
        let mut strips = HashMap::new();
        for (id, num_leds) in strips_config {
            strips.insert(*id, LEDStrip::new(*id, *num_leds));
        }
        Self { strips }
    }

    fn update_strip(&mut self, packet: &LEDPacket) {
        if let Some(strip) = self.strips.get_mut(&packet.device_id) {
            strip.update_from_packet(packet);
        }
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "LED Strip Emulator (Rust + GPU)".to_string(),
        window_width: 1000,
        window_height: 500,
        window_resizable: true,
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    // Configuration
    let port = 4210u16;
    let strips_config: Vec<(u8, usize)> = vec![(1, 67), (2, 83)];

    // Create shared state
    let shared_state = Arc::new(Mutex::new(SharedState::new(&strips_config)));

    // Create channel for packet communication
    let (tx, rx) = mpsc::channel::<LEDPacket>();

    // Start UDP listener in a separate thread
    let udp_listener = UDPListener::new(port, tx);
    thread::spawn(move || udp_listener.run());

    // UI Constants
    let bg_color = Color::from_rgba(45, 45, 45, 255);
    let canvas_bg_color = Color::from_rgba(26, 26, 26, 255);
    let text_color = WHITE;
    let status_color = Color::from_rgba(136, 255, 136, 255);
    let section_border_color = Color::from_rgba(80, 80, 80, 255);
    let section_bg_color = Color::from_rgba(35, 35, 35, 255);
    let button_color = Color::from_rgba(200, 60, 60, 255);
    let button_hover_color = Color::from_rgba(220, 80, 80, 255);
    let button_text_color = WHITE;

    let led_size = 8.0;
    let led_spacing = 2.0;
    let strip_padding = 20.0;
    let header_height = 60.0;
    let controls_height = 80.0;
    let section_padding = 15.0;

    println!("LED Strip Emulator started");
    println!("Listening on UDP port {}", port);
    println!("Strips: {:?}", strips_config);
    println!("");
    println!("Close window or click 'Stop Emulator' to exit");

    loop {
        // Process pending packets
        while let Ok(packet) = rx.try_recv() {
            if let Ok(mut state) = shared_state.lock() {
                state.update_strip(&packet);
            }
        }

        // Get window dimensions
        let screen_w = screen_width();
        let screen_h = screen_height();

        // Clear background
        clear_background(bg_color);

        // Calculate LED area height
        let led_area_height = screen_h - controls_height;

        // Draw header
        let title_text = "LED Strip Emulator (Rust + GPU)";
        let title_size = 24;
        let title_dims = measure_text(title_text, None, title_size, 1.0);
        draw_text(
            title_text,
            (screen_w - title_dims.width) / 2.0,
            35.0,
            title_size as f32,
            text_color,
        );

        // Draw port info
        let port_text = format!("Listening on UDP port {}", port);
        let port_size = 14;
        let port_dims = measure_text(&port_text, None, port_size, 1.0);
        draw_text(
            &port_text,
            (screen_w - port_dims.width) / 2.0,
            55.0,
            port_size as f32,
            Color::from_rgba(180, 180, 180, 255),
        );

        // Draw LED strips
        if let Ok(state) = shared_state.lock() {
            let num_strips = strips_config.len();
            let section_height =
                (led_area_height - header_height - strip_padding * 2.0) / num_strips as f32;

            for (i, (strip_id, num_leds)) in strips_config.iter().enumerate() {
                if let Some(strip) = state.strips.get(strip_id) {
                    let section_y = header_height + strip_padding + (i as f32 * section_height);
                    let section_w = screen_w - strip_padding * 2.0;

                    // Draw section background
                    draw_rectangle(
                        strip_padding,
                        section_y,
                        section_w,
                        section_height - strip_padding,
                        section_bg_color,
                    );

                    // Draw section border
                    draw_rectangle_lines(
                        strip_padding,
                        section_y,
                        section_w,
                        section_height - strip_padding,
                        1.0,
                        section_border_color,
                    );

                    // Draw strip label
                    let label_text = format!("Strip {} ({} LEDs)", strip_id, num_leds);
                    let label_size = 14;
                    draw_text(
                        &label_text,
                        strip_padding + section_padding,
                        section_y + 25.0,
                        label_size as f32,
                        text_color,
                    );

                    // Calculate LED layout
                    let led_area_y = section_y + 35.0;
                    let led_area_h = section_height - strip_padding - 60.0;
                    let led_total_width = *num_leds as f32 * (led_size + led_spacing);
                    let led_start_x = (screen_w - led_total_width) / 2.0;
                    let led_y = led_area_y + (led_area_h - led_size) / 2.0;

                    // Draw canvas background for LED area
                    draw_rectangle(
                        led_start_x - led_spacing,
                        led_y - led_spacing,
                        led_total_width + led_spacing * 2.0,
                        led_size + led_spacing * 2.0,
                        canvas_bg_color,
                    );

                    // Draw LEDs
                    let brightness_factor = strip.brightness as f32 / 255.0;
                    for (led_idx, (r, g, b)) in strip.colors.iter().enumerate() {
                        let led_x = led_start_x + (led_idx as f32 * (led_size + led_spacing));

                        // Apply brightness
                        let rf = (*r as f32 * brightness_factor) as u8;
                        let gf = (*g as f32 * brightness_factor) as u8;
                        let bf = (*b as f32 * brightness_factor) as u8;

                        let led_color = Color::from_rgba(rf, gf, bf, 255);

                        // Draw glow effect for bright LEDs
                        let max_val = rf.max(gf).max(bf);
                        if max_val > 50 {
                            for glow_layer in (1..=2).rev() {
                                let glow_factor = 0.3 * (3 - glow_layer) as f32 / 2.0;
                                let glow_r = (rf as f32 * glow_factor).min(255.0) as u8;
                                let glow_g = (gf as f32 * glow_factor).min(255.0) as u8;
                                let glow_b = (bf as f32 * glow_factor).min(255.0) as u8;
                                let glow_size = led_size + glow_layer as f32 * 3.0;
                                let glow_alpha = (100.0 * glow_factor) as u8;

                                draw_rectangle(
                                    led_x + led_size / 2.0 - glow_size / 2.0,
                                    led_y + led_size / 2.0 - glow_size / 2.0,
                                    glow_size,
                                    glow_size,
                                    Color::from_rgba(glow_r, glow_g, glow_b, glow_alpha),
                                );
                            }
                        }

                        // Draw main LED
                        draw_rectangle(led_x, led_y, led_size, led_size, led_color);

                        // Draw highlight for bright LEDs
                        if max_val > 80 {
                            let highlight_size = led_size * 0.5;
                            let highlight_r = (rf as f32 * 1.3).min(255.0) as u8;
                            let highlight_g = (gf as f32 * 1.3).min(255.0) as u8;
                            let highlight_b = (bf as f32 * 1.3).min(255.0) as u8;
                            draw_rectangle(
                                led_x + led_size / 2.0 - highlight_size / 2.0,
                                led_y + led_size / 2.0 - highlight_size / 2.0,
                                highlight_size,
                                highlight_size,
                                Color::from_rgba(highlight_r, highlight_g, highlight_b, 200),
                            );
                        }
                    }

                    // Draw status text
                    let status_text = format!(
                        "Strip {}: seq={}, brightness={}, packets={}",
                        strip_id, strip.sequence, strip.brightness, strip.packet_count
                    );
                    let status_size = 11;
                    draw_text(
                        &status_text,
                        strip_padding + section_padding,
                        section_y + section_height - strip_padding - 10.0,
                        status_size as f32,
                        status_color,
                    );
                }
            }
        }

        // Draw controls panel background
        draw_rectangle(
            0.0,
            led_area_height,
            screen_w,
            controls_height,
            section_bg_color,
        );
        draw_line(
            0.0,
            led_area_height,
            screen_w,
            led_area_height,
            2.0,
            section_border_color,
        );

        // Draw Stop/Exit button
        let button_w = 140.0;
        let button_h = 40.0;
        let button_x = (screen_w - button_w) / 2.0;
        let button_y = led_area_height + (controls_height - button_h) / 2.0;

        // Check if mouse is over button
        let (mouse_x, mouse_y) = mouse_position();
        let is_hover = mouse_x >= button_x
            && mouse_x <= button_x + button_w
            && mouse_y >= button_y
            && mouse_y <= button_y + button_h;

        let current_button_color = if is_hover {
            button_hover_color
        } else {
            button_color
        };

        // Draw button
        draw_rectangle(button_x, button_y, button_w, button_h, current_button_color);
        draw_rectangle_lines(button_x, button_y, button_w, button_h, 2.0, WHITE);

        // Draw button text
        let button_text = "Stop Emulator";
        let button_text_size = 16;
        let button_text_dims = measure_text(button_text, None, button_text_size, 1.0);
        draw_text(
            button_text,
            button_x + (button_w - button_text_dims.width) / 2.0,
            button_y + (button_h + button_text_dims.height) / 2.0 - 3.0,
            button_text_size as f32,
            button_text_color,
        );

        // Handle button click
        if is_mouse_button_pressed(MouseButton::Left) && is_hover {
            println!("Stop button clicked, exiting...");
            break;
        }

        // Handle window close (ESC key)
        if is_key_pressed(KeyCode::Escape) {
            println!("ESC pressed, exiting...");
            break;
        }

        next_frame().await;
    }

    println!("Emulator shutting down...");
}
