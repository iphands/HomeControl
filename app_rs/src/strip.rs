use std::net::UdpSocket;
use std::sync::{Arc, Mutex};

pub const PROTOCOL_SKIP: usize = 5;

pub struct Strip {
    pub dev_id: u8,
    pub udp_ip: String,
    pub udp_port: u16,
    pub num_leds: usize,
    pub leds: Vec<u8>,
    pub seq: Arc<Mutex<u8>>,
    pub socket: UdpSocket,
}

impl Strip {
    pub fn new(dev_id: u8, hostname: &str, num_leds: usize) -> Self {
        let total = (num_leds * 3) + PROTOCOL_SKIP;
        let mut leds = vec![0u8; total];

        // Initialize protocol header
        leds[0] = 1; // Message type
        leds[1] = 0; // Sequence (will be set in get_packet)
        leds[2] = 255; // Brightness
        leds[3] = num_leds as u8; // Number of LEDs
        leds[4] = dev_id; // Device ID

        let socket = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind UDP socket");
        socket.set_nonblocking(true).expect("Failed to set non-blocking");

        Self {
            dev_id,
            udp_ip: hostname.to_string(),
            udp_port: 4210,
            num_leds,
            leds,
            seq: Arc::new(Mutex::new(0)),
            socket,
        }
    }

    pub fn get_brightness(&self) -> u8 {
        self.leds[2]
    }

    pub fn set_brightness(&mut self, val: u8) {
        self.leds[2] = val;
    }

    pub fn set_led(&mut self, led: usize, r: u8, g: u8, b: u8) {
        if led < self.num_leds {
            let offset = (led * 3) + PROTOCOL_SKIP;
            self.leds[offset] = r;
            self.leds[offset + 1] = g;
            self.leds[offset + 2] = b;
        }
    }

    pub fn set_led_arr(&mut self, led: usize, arr: [u8; 3]) {
        self.set_led(led, arr[0], arr[1], arr[2]);
    }

    pub fn get_led(&self, led: usize) -> Option<[u8; 3]> {
        if led < self.num_leds {
            let offset = (led * 3) + PROTOCOL_SKIP;
            Some([self.leds[offset], self.leds[offset + 1], self.leds[offset + 2]])
        } else {
            None
        }
    }

    pub fn vol(&self, rgb: [u8; 3], pct: f64) -> [u8; 3] {
        [
            (rgb[0] as f64 * pct) as u8,
            (rgb[1] as f64 * pct) as u8,
            (rgb[2] as f64 * pct) as u8,
        ]
    }

    pub fn get_packet(&mut self) -> Vec<u8> {
        let mut seq = self.seq.lock().unwrap();
        self.leds[1] = *seq;
        *seq = seq.wrapping_add(1);
        self.leds.clone()
    }

    pub fn send(&mut self) {
        let packet = self.get_packet();
        let _ = self.socket.send_to(&packet, (&self.udp_ip[..], self.udp_port));
    }

    pub fn solid(&mut self, arr: [u8; 3]) {
        for x in 0..self.num_leds {
            self.set_led_arr(x, arr);
        }
        self.send();
    }

    pub fn get_rgb(&self, h: f64, s: f64, v: f64) -> [u8; 3] {
        // HSV to RGB conversion
        let c = v * s;
        let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
        let m = v - c;

        let (r, g, b) = if h < 1.0 / 6.0 {
            (c, x, 0.0)
        } else if h < 2.0 / 6.0 {
            (x, c, 0.0)
        } else if h < 3.0 / 6.0 {
            (0.0, c, x)
        } else if h < 4.0 / 6.0 {
            (0.0, x, c)
        } else if h < 5.0 / 6.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };

        [((r + m) * 255.0) as u8, ((g + m) * 255.0) as u8, ((b + m) * 255.0) as u8]
    }
}
