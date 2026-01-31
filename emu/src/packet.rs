use anyhow::{anyhow, Result};
use std::net::SocketAddr;

/// LED strip packet structure matching the ESP32 protocol
#[derive(Debug, Clone)]
pub struct LEDPacket {
    pub msg_type: u8,
    pub sequence: u8,
    pub brightness: u8,
    pub num_leds: u8,
    pub device_id: u8,
    pub led_data: Vec<[u8; 3]>,
    pub source_addr: SocketAddr,
}

impl LEDPacket {
    /// Parse a UDP packet from bytes according to the LED strip protocol
    ///
    /// Packet format:
    /// Byte 0: Message type (1 = LED_STRIP)
    /// Byte 1: Sequence number (0-255)
    /// Byte 2: Brightness (0-255)
    /// Byte 3: Number of LEDs
    /// Byte 4: Device ID
    /// Bytes 5+: RGB data (3 bytes per LED)
    pub fn from_bytes(data: &[u8], source_addr: SocketAddr) -> Result<Self> {
        if data.len() < 5 {
            return Err(anyhow!("Packet too short: {} bytes", data.len()));
        }

        let msg_type = data[0];
        let sequence = data[1];
        let brightness = data[2];
        let num_leds = data[3];
        let device_id = data[4];

        let expected_len = 5 + (num_leds as usize) * 3;
        if data.len() < expected_len {
            return Err(anyhow!(
                "Packet too short for {} LEDs: expected {} bytes, got {}",
                num_leds,
                expected_len,
                data.len()
            ));
        }

        let mut led_data = Vec::with_capacity(num_leds as usize);
        for i in 0..num_leds {
            let offset = 5 + (i as usize) * 3;
            let r = data[offset];
            let g = data[offset + 1];
            let b = data[offset + 2];
            led_data.push([r, g, b]);
        }

        Ok(LEDPacket {
            msg_type,
            sequence,
            brightness,
            num_leds,
            device_id,
            led_data,
            source_addr,
        })
    }

    /// Get RGB value for a specific LED
    pub fn get_led(&self, index: usize) -> Option<[u8; 3]> {
        if index < self.led_data.len() {
            Some(self.led_data[index])
        } else {
            None
        }
    }

    /// Check if any LED has a non-zero value
    pub fn has_any_lit(&self) -> bool {
        self.led_data
            .iter()
            .any(|[r, g, b]| *r > 0 || *g > 0 || *b > 0)
    }

    /// Count how many LEDs have any non-zero value
    pub fn count_lit(&self) -> usize {
        self.led_data
            .iter()
            .filter(|[r, g, b]| *r > 0 || *g > 0 || *b > 0)
            .count()
    }

    /// Get LED data as a vector of RGB tuples with brightness applied
    pub fn get_led_data_with_brightness(&self) -> Vec<(u8, u8, u8)> {
        let brightness_factor = self.brightness as f32 / 255.0;
        self.led_data
            .iter()
            .map(|[r, g, b]| {
                let rf = (*r as f32 * brightness_factor).round() as u8;
                let gf = (*g as f32 * brightness_factor).round() as u8;
                let bf = (*b as f32 * brightness_factor).round() as u8;
                (rf, gf, bf)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn test_addr() -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 4210)
    }

    #[test]
    fn test_parse_minimal_packet() {
        let data = vec![1, 0, 255, 0, 1]; // msg_type=1, seq=0, brightness=255, num_leds=0, device_id=1
        let packet = LEDPacket::from_bytes(&data, test_addr()).unwrap();

        assert_eq!(packet.msg_type, 1);
        assert_eq!(packet.sequence, 0);
        assert_eq!(packet.brightness, 255);
        assert_eq!(packet.num_leds, 0);
        assert_eq!(packet.device_id, 1);
        assert_eq!(packet.led_data.len(), 0);
    }

    #[test]
    fn test_parse_single_led_packet() {
        let data = vec![
            1,   // msg_type
            42,  // sequence
            128, // brightness
            1,   // num_leds
            2,   // device_id
            255, // R
            0,   // G
            128, // B
        ];
        let packet = LEDPacket::from_bytes(&data, test_addr()).unwrap();

        assert_eq!(packet.msg_type, 1);
        assert_eq!(packet.sequence, 42);
        assert_eq!(packet.brightness, 128);
        assert_eq!(packet.num_leds, 1);
        assert_eq!(packet.device_id, 2);
        assert_eq!(packet.led_data.len(), 1);
        assert_eq!(packet.get_led(0), Some([255, 0, 128]));
    }

    #[test]
    fn test_brightness_application() {
        let data = vec![
            1, 0, 128, 1, 1, // header with brightness 128/255
            255, 255, 255, // white LED at full brightness in data
        ];
        let packet = LEDPacket::from_bytes(&data, test_addr()).unwrap();
        let led_data = packet.get_led_data_with_brightness();

        // With brightness 128/255 ~ 0.5, white should become ~ (128, 128, 128)
        assert_eq!(led_data[0], (128, 128, 128));
    }

    #[test]
    fn test_packet_too_short() {
        let data = vec![1, 0, 255]; // Missing required header bytes
        assert!(LEDPacket::from_bytes(&data, test_addr()).is_err());
    }

    #[test]
    fn test_insufficient_led_data() {
        let data = vec![
            1,   // msg_type
            0,   // sequence
            255, // brightness
            2,   // num_leds = 2
            1,   // device_id
            255, 0, 128, // LED 0 data
                 // Missing LED 1 data - should fail
        ];
        assert!(LEDPacket::from_bytes(&data, test_addr()).is_err());
    }
}
