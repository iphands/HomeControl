use crate::packet::LEDPacket;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, RwLock};

/// Configuration for a single LED strip
#[derive(Debug, Clone)]
pub struct StripConfig {
    pub id: u8,
    pub num_leds: u8,
    pub port: u16,
}

impl Default for StripConfig {
    fn default() -> Self {
        Self {
            id: 1,
            num_leds: 67,
            port: 4210,
        }
    }
}

/// Message sent from UDP listener to the main application
#[derive(Debug, Clone)]
pub enum StripMessage {
    PacketReceived(LEDPacket),
    ListenerError(String),
}

/// UDP listener that handles multiple strips on different ports
#[derive(Clone)]
pub struct UDPListener {
    strips: Vec<StripConfig>,
    socket: Arc<UdpSocket>,
    message_sender: mpsc::UnboundedSender<StripMessage>,
    packet_count: Arc<RwLock<HashMap<u8, usize>>>, // device_id -> count
}

impl UDPListener {
    /// Create a new UDP listener for the given strip configurations
    pub fn new(
        strips: Vec<StripConfig>,
        message_sender: mpsc::UnboundedSender<StripMessage>,
    ) -> Result<Self> {
        // For now, we'll bind to the first port and handle all packets
        // In a more complex setup, we might need multiple sockets
        let bind_port = strips
            .first()
            .ok_or_else(|| anyhow!("No strip configurations provided"))?
            .port;

        let socket = std::net::UdpSocket::bind(("0.0.0.0", bind_port))
            .map_err(|e| anyhow!("Failed to bind to port {}: {}", bind_port, e))?;

        socket.set_nonblocking(true)
            .map_err(|e| anyhow!("Failed to set non-blocking mode: {}", e))?;

        let socket = UdpSocket::from_std(socket)
            .map_err(|e| anyhow!("Failed to create async socket: {}", e))?;

        Ok(Self {
            strips,
            socket: Arc::new(socket),
            message_sender,
            packet_count: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Start listening for UDP packets
    pub async fn start(&self) -> Result<()> {
        let mut buf = vec![0u8; 4096]; // Buffer large enough for any reasonable packet

        println!("UDP Listener started on port {}", self.socket.local_addr()?.port());

        loop {
            match self.socket.recv_from(&mut buf).await {
                Ok((len, addr)) => {
                    let data = buf[..len].to_vec();
                    let packet_count = self.packet_count.clone();
                    let sender = self.message_sender.clone();

                    // Parse packet in a separate task to avoid blocking the listener
                    tokio::spawn(async move {
                        match LEDPacket::from_bytes(&data, addr) {
                            Ok(packet) => {
                                // Update packet count
                                {
                                    let mut count = packet_count.write().await;
                                    *count.entry(packet.device_id).or_insert(0) += 1;
                                }

                                // Send packet to main application
                                if let Err(e) = sender.send(StripMessage::PacketReceived(packet)) {
                                    eprintln!("Failed to send packet to main application: {}", e);
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to parse packet from {}: {}", addr, e);
                                if let Err(e) = sender.send(StripMessage::ListenerError(format!(
                                    "Parse error from {}: {}",
                                    addr, e
                                ))) {
                                    eprintln!("Failed to send error message: {}", e);
                                }
                            }
                        }
                    });
                }
                Err(e) => {
                    eprintln!("UDP receive error: {}", e);
                    // Continue listening even if there's an error
                }
            }
        }
    }

    /// Get packet count for a specific device
    pub async fn get_packet_count(&self, device_id: u8) -> usize {
        let count = self.packet_count.read().await;
        count.get(&device_id).copied().unwrap_or(0)
    }

    /// Get all packet counts
    pub async fn get_all_packet_counts(&self) -> HashMap<u8, usize> {
        let count = self.packet_count.read().await;
        count.clone()
    }

    /// Reset packet count for a specific device
    pub async fn reset_packet_count(&self, device_id: u8) {
        let mut count = self.packet_count.write().await;
        count.remove(&device_id);
    }

    /// Reset all packet counts
    pub async fn reset_all_packet_counts(&self) {
        let mut count = self.packet_count.write().await;
        count.clear();
    }
}

/// Create default strip configurations based on the Python server setup
pub fn create_default_strips() -> Vec<StripConfig> {
    vec![
        StripConfig {
            id: 1,
            num_leds: 67,
            port: 4210, // Default port for strip 1
        },
        StripConfig {
            id: 2,
            num_leds: 83,
            port: 4210, // Same port, different device_id in packets
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_config_default() {
        let config = StripConfig::default();
        assert_eq!(config.id, 1);
        assert_eq!(config.num_leds, 67);
        assert_eq!(config.port, 4210);
    }

    #[test]
    fn test_create_default_strips() {
        let strips = create_default_strips();
        assert_eq!(strips.len(), 2);
        assert_eq!(strips[0].id, 1);
        assert_eq!(strips[0].num_leds, 67);
        assert_eq!(strips[1].id, 2);
        assert_eq!(strips[1].num_leds, 83);
    }
}