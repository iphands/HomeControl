"""
Fake ESP32 UDP listener for testing the LED controller API.

This module simulates an ESP32 device that listens for UDP packets
from the LED controller server. It captures packets for verification
in tests.
"""

import socket
import threading
import queue
from dataclasses import dataclass
from typing import List, Optional


@dataclass
class LEDPacket:
    """Parsed LED packet from the server."""
    msg_type: int
    sequence: int
    brightness: int
    num_leds: int
    device_id: int
    led_data: List[tuple]  # List of (r, g, b) tuples

    @classmethod
    def from_bytes(cls, data: bytes) -> "LEDPacket":
        """Parse a UDP packet into an LEDPacket."""
        if len(data) < 5:
            raise ValueError(f"Packet too short: {len(data)} bytes")

        msg_type = data[0]
        sequence = data[1]
        brightness = data[2]
        num_leds = data[3]
        device_id = data[4]

        led_data = []
        for i in range(num_leds):
            offset = 5 + (i * 3)
            if offset + 2 < len(data):
                r = data[offset]
                g = data[offset + 1]
                b = data[offset + 2]
                led_data.append((r, g, b))

        return cls(
            msg_type=msg_type,
            sequence=sequence,
            brightness=brightness,
            num_leds=num_leds,
            device_id=device_id,
            led_data=led_data,
        )

    def get_led(self, index: int) -> Optional[tuple]:
        """Get the RGB value for a specific LED."""
        if 0 <= index < len(self.led_data):
            return self.led_data[index]
        return None

    def has_any_lit(self) -> bool:
        """Check if any LED has a non-zero value."""
        return any(r > 0 or g > 0 or b > 0 for r, g, b in self.led_data)

    def count_lit(self) -> int:
        """Count how many LEDs have any non-zero value."""
        return sum(1 for r, g, b in self.led_data if r > 0 or g > 0 or b > 0)


class FakeESP32:
    """Simulates an ESP32 device listening for LED control packets."""

    def __init__(self, device_id: int, port: int = 4210, host: str = "0.0.0.0"):
        self.device_id = device_id
        self.port = port
        self.host = host
        self.packets: queue.Queue[LEDPacket] = queue.Queue()
        self._running = False
        self._thread: Optional[threading.Thread] = None
        self._socket: Optional[socket.socket] = None

    def start(self):
        """Start listening for UDP packets."""
        if self._running:
            return

        self._socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self._socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self._socket.bind((self.host, self.port))
        self._socket.settimeout(0.5)  # Allow checking for shutdown
        self._running = True
        self._ready = threading.Event()
        self._thread = threading.Thread(target=self._listen_loop, daemon=True)
        self._thread.start()
        # Wait for thread to be ready
        self._ready.wait(timeout=2.0)

    def stop(self):
        """Stop listening for UDP packets."""
        self._running = False
        if self._thread:
            self._thread.join(timeout=2.0)
        if self._socket:
            self._socket.close()
            self._socket = None

    def _listen_loop(self):
        """Main loop for receiving packets."""
        self._ready.set()  # Signal that we're ready to receive
        while self._running:
            try:
                data, addr = self._socket.recvfrom(1024)
                packet = LEDPacket.from_bytes(data)
                # Only capture packets meant for this device
                if packet.device_id == self.device_id:
                    self.packets.put(packet)
            except socket.timeout:
                continue
            except Exception as e:
                if self._running:
                    print(f"Error receiving packet: {e}")

    def get_packet(self, timeout: float = 2.0) -> Optional[LEDPacket]:
        """Get the next received packet, with timeout."""
        try:
            return self.packets.get(timeout=timeout)
        except queue.Empty:
            return None

    def get_all_packets(self, timeout: float = 0.1) -> List[LEDPacket]:
        """Get all currently queued packets."""
        packets = []
        while True:
            try:
                packets.append(self.packets.get(timeout=timeout))
            except queue.Empty:
                break
        return packets

    def clear_packets(self):
        """Clear all queued packets."""
        while not self.packets.empty():
            try:
                self.packets.get_nowait()
            except queue.Empty:
                break

    def wait_for_packets(self, count: int, timeout: float = 5.0) -> List[LEDPacket]:
        """Wait for a specific number of packets."""
        packets = []
        import time
        deadline = time.time() + timeout
        while len(packets) < count and time.time() < deadline:
            remaining = deadline - time.time()
            packet = self.get_packet(timeout=min(0.5, remaining))
            if packet:
                packets.append(packet)
        return packets

    def __enter__(self):
        self.start()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.stop()
