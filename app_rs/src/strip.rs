//! LED strip hardware communication module.
//!
//! This module provides UDP-based communication with ESP32 LED controllers.
//! It handles packet construction, brightness control, and efficient LED updates.

use std::net::UdpSocket;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::error::{Error, Result};

/// Size of the protocol header in bytes.
const PROTOCOL_HEADER_SIZE: usize = 5;

/// Protocol message type for LED data.
const MSG_TYPE_LED_DATA: u8 = 1;

/// Default UDP port for ESP32 LED controllers.
const DEFAULT_UDP_PORT: u16 = 4210;

/// Represents a single LED strip connected via UDP.
///
/// The `Strip` struct manages the LED buffer and UDP communication for a
/// single physical LED strip. It handles protocol headers, brightness scaling,
/// and color space conversions.
///
/// # Protocol Format
/// Each UDP packet has a 5-byte header followed by RGB data:
/// - Byte 0: Message type (1 = LED data)
/// - Byte 1: Sequence number (for packet ordering)
/// - Byte 2: Global brightness (0-255)
/// - Byte 3: Number of LEDs
/// - Byte 4: Device ID
/// - Bytes 5+: RGB triplets for each LED
pub struct Strip {
    /// Device identifier (0-255).
    pub dev_id: u8,

    /// Target hostname or IP address.
    pub udp_ip: String,

    /// Target UDP port.
    pub udp_port: u16,

    /// Number of LEDs on this strip.
    pub num_leds: usize,

    /// Internal buffer containing protocol header + RGB data.
    buffer: Vec<u8>,

    /// Sequence counter for packet ordering (atomic for thread safety).
    seq: AtomicU8,

    /// UDP socket for communication.
    socket: UdpSocket,
}

impl Strip {
    /// Creates a new LED strip connection.
    ///
    /// # Arguments
    /// * `dev_id` - Unique device identifier
    /// * `hostname` - Target hostname or IP address
    /// * `num_leds` - Number of LEDs on the strip
    ///
    /// # Errors
    /// Returns an error if the UDP socket cannot be created or configured.
    ///
    /// # Example
    /// ```
    /// let strip = Strip::new(1, "esp32c6-00.lan", 67)?;
    /// ```
    pub fn new(dev_id: u8, hostname: &str, num_leds: usize) -> Result<Self> {
        let total_size = (num_leds * 3) + PROTOCOL_HEADER_SIZE;
        let mut buffer = vec![0u8; total_size];

        // Initialize protocol header
        buffer[0] = MSG_TYPE_LED_DATA;
        buffer[1] = 0; // Sequence (set dynamically)
        buffer[2] = 255; // Default brightness (max)
        buffer[3] = num_leds as u8;
        buffer[4] = dev_id;

        let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| Error::Network(e.to_string()))?;

        socket.set_nonblocking(true).map_err(|e| Error::Network(e.to_string()))?;

        Ok(Self {
            dev_id,
            udp_ip: hostname.to_string(),
            udp_port: DEFAULT_UDP_PORT,
            num_leds,
            buffer,
            seq: AtomicU8::new(0),
            socket,
        })
    }

    /// Returns the current global brightness level (0-255).
    pub fn brightness(&self) -> u8 {
        self.buffer[2]
    }

    /// Sets the global brightness level for the entire strip.
    ///
    /// # Arguments
    /// * `val` - Brightness value in range 0-255
    pub fn set_brightness(&mut self, val: u8) {
        self.buffer[2] = val;
    }

    /// Sets a single LED to the specified RGB color.
    ///
    /// # Arguments
    /// * `led` - LED index (0-based)
    /// * `r` - Red component (0-255)
    /// * `g` - Green component (0-255)
    /// * `b` - Blue component (0-255)
    ///
    /// # Panics
    /// Panics in debug mode if `led` is out of bounds.
    pub fn set_led(&mut self, led: usize, r: u8, g: u8, b: u8) {
        debug_assert!(led < self.num_leds, "LED index {led} out of bounds");

        if led < self.num_leds {
            let offset = (led * 3) + PROTOCOL_HEADER_SIZE;
            self.buffer[offset] = r;
            self.buffer[offset + 1] = g;
            self.buffer[offset + 2] = b;
        }
    }

    /// Sets a single LED using an RGB array.
    ///
    /// Convenience wrapper around [`set_led`](Self::set_led).
    pub fn set_led_rgb(&mut self, led: usize, rgb: [u8; 3]) {
        self.set_led(led, rgb[0], rgb[1], rgb[2]);
    }

    /// Gets the current RGB value of a specific LED.
    ///
    /// Returns `None` if the LED index is out of bounds.
    pub fn get_led(&self, led: usize) -> Option<[u8; 3]> {
        if led < self.num_leds {
            let offset = (led * 3) + PROTOCOL_HEADER_SIZE;
            Some([self.buffer[offset], self.buffer[offset + 1], self.buffer[offset + 2]])
        } else {
            None
        }
    }

    /// Scales an RGB color by a percentage.
    ///
    /// Useful for creating fade effects or applying brightness
    /// scaling to individual colors before sending to the strip.
    ///
    /// # Arguments
    /// * `rgb` - Input RGB color
    /// * `pct` - Scaling factor (0.0 to 1.0)
    pub fn scale_color(&self, rgb: [u8; 3], pct: f64) -> [u8; 3] {
        let scale = |v: u8| (v as f64 * pct).clamp(0.0, 255.0) as u8;
        [scale(rgb[0]), scale(rgb[1]), scale(rgb[2])]
    }

    /// Returns a complete packet ready for transmission.
    ///
    /// This method updates the sequence number and returns a copy
    /// of the buffer for sending over UDP.
    pub fn packet(&mut self) -> Vec<u8> {
        // Update sequence number atomically
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        self.buffer[1] = seq;
        self.buffer.clone()
    }

    /// Sends the current LED state to the physical strip.
    ///
    /// Non-blocking operation - failures are silently ignored
    /// to prevent animation stuttering from network issues.
    pub fn send(&mut self) {
        let packet = self.packet();
        // Non-blocking send - ignore errors
        let _ = self.socket.send_to(&packet, (&self.udp_ip[..], self.udp_port));
    }

    /// Fills the entire strip with a single color.
    ///
    /// # Arguments
    /// * `rgb` - RGB color to fill with
    pub fn fill(&mut self, rgb: [u8; 3]) {
        for i in 0..self.num_leds {
            self.set_led_rgb(i, rgb);
        }
        self.send();
    }

    /// Converts HSV color space to RGB.
    ///
    /// # Arguments
    /// * `h` - Hue in range 0.0 to 1.0
    /// * `s` - Saturation in range 0.0 to 1.0
    /// * `v` - Value (brightness) in range 0.0 to 1.0
    ///
    /// # Returns
    /// RGB color as `[u8; 3]` array
    pub fn hsv_to_rgb(&self, h: f64, s: f64, v: f64) -> [u8; 3] {
        let c = v * s;
        let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
        let m = v - c;

        let (r, g, b) = match (h * 6.0) as u8 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };

        [((r + m) * 255.0) as u8, ((g + m) * 255.0) as u8, ((b + m) * 255.0) as u8]
    }

    /// Returns the full RGB color spectrum for this strip length.
    ///
    /// Generates a rainbow gradient distributed evenly across all LEDs.
    pub fn rainbow_colors(&self) -> Vec<[u8; 3]> {
        let step = 1.0 / self.num_leds as f64;
        (0..self.num_leds)
            .map(|i| self.hsv_to_rgb(i as f64 * step, 1.0, 1.0))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_creation() {
        let strip = Strip::new(1, "127.0.0.1", 10).unwrap();
        assert_eq!(strip.dev_id, 1);
        assert_eq!(strip.num_leds, 10);
        assert_eq!(strip.brightness(), 255);
    }

    #[test]
    fn test_brightness_control() {
        let mut strip = Strip::new(1, "127.0.0.1", 10).unwrap();
        strip.set_brightness(128);
        assert_eq!(strip.brightness(), 128);
    }

    #[test]
    fn test_led_operations() {
        let mut strip = Strip::new(1, "127.0.0.1", 10).unwrap();

        // Set and get LED
        strip.set_led(0, 255, 128, 64);
        assert_eq!(strip.get_led(0), Some([255, 128, 64]));

        // RGB array interface
        strip.set_led_rgb(1, [100, 200, 50]);
        assert_eq!(strip.get_led(1), Some([100, 200, 50]));

        // Out of bounds returns None
        assert_eq!(strip.get_led(100), None);
    }

    #[test]
    fn test_color_scaling() {
        let strip = Strip::new(1, "127.0.0.1", 10).unwrap();

        let scaled = strip.scale_color([255, 128, 64], 0.5);
        assert_eq!(scaled[0], 127);
        assert_eq!(scaled[1], 64);
        assert_eq!(scaled[2], 32);
    }

    #[test]
    fn test_hsv_to_rgb() {
        let strip = Strip::new(1, "127.0.0.1", 10).unwrap();

        // Red
        assert_eq!(strip.hsv_to_rgb(0.0, 1.0, 1.0), [255, 0, 0]);

        // Green
        assert_eq!(strip.hsv_to_rgb(1.0 / 3.0, 1.0, 1.0), [0, 255, 0]);

        // Blue
        assert_eq!(strip.hsv_to_rgb(2.0 / 3.0, 1.0, 1.0), [0, 0, 255]);
    }

    #[test]
    fn test_rainbow_generation() {
        let strip = Strip::new(1, "127.0.0.1", 10).unwrap();
        let colors = strip.rainbow_colors();

        assert_eq!(colors.len(), 10);
        // First should be red-ish
        assert!(colors[0][0] > 200);
        // Middle should be cyan/green-ish
        assert!(colors[5][1] > 100 || colors[5][2] > 100);
    }

    #[test]
    fn test_packet_generation() {
        let mut strip = Strip::new(1, "127.0.0.1", 3).unwrap();
        strip.set_led_rgb(0, [255, 0, 0]);

        let packet1 = strip.packet();
        let packet2 = strip.packet();

        // Sequence numbers should increment
        assert_eq!(packet1[1], 0);
        assert_eq!(packet2[1], 1);

        // Header should be consistent
        assert_eq!(packet1[0], MSG_TYPE_LED_DATA);
        assert_eq!(packet1[2], 255); // Brightness
        assert_eq!(packet1[3], 3); // LED count
        assert_eq!(packet1[4], 1); // Device ID
    }
}
