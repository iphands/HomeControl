#!/usr/bin/env python3
"""
LED Strip Emulator - Visual simulator for LED strip output.

This program listens for UDP packets from the LED controller server
and displays the LED colors in a GUI window.
"""

import sys
import os
import socket
import threading
import tkinter as tk
from tkinter import ttk
from dataclasses import dataclass
from typing import List, Optional, Callable

# Add parent directory to path to import from tests_external
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from fake_esp32 import LEDPacket


class LEDStripWidget(tk.Canvas):
    """Canvas widget that displays an LED strip."""

    def __init__(self, parent, num_leds: int, strip_id: int, led_size: int = 8, **kwargs):
        self.num_leds = num_leds
        self.strip_id = strip_id
        self.led_size = led_size
        self.led_spacing = 2
        self.brightness = 255

        # Calculate dimensions
        width = (led_size + self.led_spacing) * num_leds + self.led_spacing
        height = led_size + self.led_spacing * 2

        super().__init__(parent, width=width, height=height, bg='#1a1a1a', **kwargs)

        # Store LED rectangle IDs
        self.led_rects = []
        self._create_leds()

        # Current colors (default to off)
        self.colors = [(0, 0, 0)] * num_leds

    def _create_leds(self):
        """Create the LED rectangle objects."""
        for i in range(self.num_leds):
            x = self.led_spacing + i * (self.led_size + self.led_spacing)
            y = self.led_spacing
            rect = self.create_rectangle(
                x, y, x + self.led_size, y + self.led_size,
                fill='#000000', outline='#333333', width=1
            )
            self.led_rects.append(rect)

    def update_leds(self, colors: List[tuple], brightness: int = 255):
        """Update LED colors from a list of (r, g, b) tuples."""
        self.brightness = brightness
        self.colors = colors

        for i, rect in enumerate(self.led_rects):
            if i < len(colors):
                r, g, b = colors[i]
                # Apply brightness
                r = int(r * brightness / 255)
                g = int(g * brightness / 255)
                b = int(b * brightness / 255)
                color = f'#{r:02x}{g:02x}{b:02x}'
            else:
                color = '#000000'
            self.itemconfig(rect, fill=color)


class UDPListener:
    """Listens for UDP packets and dispatches to handlers."""

    def __init__(self, port: int, host: str = "0.0.0.0"):
        self.port = port
        self.host = host
        self._running = False
        self._thread: Optional[threading.Thread] = None
        self._socket: Optional[socket.socket] = None
        self._handlers: dict[int, Callable] = {}  # device_id -> callback

    def add_handler(self, device_id: int, callback: Callable[[LEDPacket], None]):
        """Register a callback for packets with a specific device ID."""
        self._handlers[device_id] = callback

    def start(self):
        """Start listening for UDP packets."""
        if self._running:
            return

        self._socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self._socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self._socket.bind((self.host, self.port))
        self._socket.settimeout(0.5)
        self._running = True
        self._thread = threading.Thread(target=self._listen_loop, daemon=True)
        self._thread.start()

    def stop(self):
        """Stop listening."""
        self._running = False
        if self._thread:
            self._thread.join(timeout=2.0)
        if self._socket:
            self._socket.close()
            self._socket = None

    def _listen_loop(self):
        """Main receive loop."""
        while self._running:
            try:
                data, addr = self._socket.recvfrom(1024)
                packet = LEDPacket.from_bytes(data)
                if packet.device_id in self._handlers:
                    self._handlers[packet.device_id](packet)
            except socket.timeout:
                continue
            except Exception as e:
                if self._running:
                    print(f"Error receiving packet: {e}")


class LEDStripEmulator:
    """Main emulator application."""

    def __init__(self, strips_config: List[dict], port: int = 4210):
        """
        Initialize the emulator.

        Args:
            strips_config: List of dicts with 'id' and 'num_leds' keys
            port: UDP port to listen on
        """
        self.strips_config = strips_config
        self.port = port
        self.root: Optional[tk.Tk] = None
        self.strip_widgets: dict[int, LEDStripWidget] = {}
        self.listener: Optional[UDPListener] = None
        self.status_labels: dict[int, tk.Label] = {}
        self.packet_counts: dict[int, int] = {}

    def _on_packet(self, strip_id: int, packet: LEDPacket):
        """Handle received packet by updating the corresponding strip."""
        if strip_id in self.strip_widgets:
            # Schedule GUI update on main thread
            self.root.after(0, lambda: self._update_strip(strip_id, packet))

    def _update_strip(self, strip_id: int, packet: LEDPacket):
        """Update strip widget (must be called from main thread)."""
        widget = self.strip_widgets.get(strip_id)
        if widget:
            widget.update_leds(packet.led_data, packet.brightness)

            # Update packet count
            self.packet_counts[strip_id] = self.packet_counts.get(strip_id, 0) + 1
            if strip_id in self.status_labels:
                self.status_labels[strip_id].config(
                    text=f"Strip {strip_id}: seq={packet.sequence}, brightness={packet.brightness}, packets={self.packet_counts[strip_id]}"
                )

    def _on_stop(self):
        """Handle stop button click."""
        self.stop()

    def start(self):
        """Start the emulator GUI and UDP listener."""
        # Create main window
        self.root = tk.Tk()
        self.root.title("LED Strip Emulator")
        self.root.configure(bg='#2d2d2d')
        self.root.resizable(True, True)

        # Style
        style = ttk.Style()
        style.theme_use('clam')
        style.configure('TFrame', background='#2d2d2d')
        style.configure('TLabel', background='#2d2d2d', foreground='#ffffff')
        style.configure('TButton', padding=10)
        style.configure('Header.TLabel', font=('Helvetica', 14, 'bold'))
        style.configure('Status.TLabel', font=('Courier', 10), foreground='#88ff88')

        # Main frame
        main_frame = ttk.Frame(self.root, padding=20)
        main_frame.pack(fill=tk.BOTH, expand=True)

        # Header
        header = ttk.Label(main_frame, text="LED Strip Emulator", style='Header.TLabel')
        header.pack(pady=(0, 10))

        # Port info
        port_label = ttk.Label(main_frame, text=f"Listening on UDP port {self.port}")
        port_label.pack(pady=(0, 20))

        # Create strip displays
        for config in self.strips_config:
            strip_id = config['id']
            num_leds = config['num_leds']

            # Strip frame
            strip_frame = ttk.Frame(main_frame)
            strip_frame.pack(fill=tk.X, pady=10)

            # Strip label
            label = ttk.Label(strip_frame, text=f"Strip {strip_id} ({num_leds} LEDs)")
            label.pack(anchor=tk.W)

            # LED strip widget
            strip_widget = LEDStripWidget(strip_frame, num_leds, strip_id)
            strip_widget.pack(pady=5)
            self.strip_widgets[strip_id] = strip_widget

            # Status label
            status = ttk.Label(strip_frame, text=f"Strip {strip_id}: waiting for packets...", style='Status.TLabel')
            status.pack(anchor=tk.W)
            self.status_labels[strip_id] = status

        # Separator
        ttk.Separator(main_frame, orient=tk.HORIZONTAL).pack(fill=tk.X, pady=20)

        # Stop button
        stop_btn = ttk.Button(main_frame, text="Stop Emulator", command=self._on_stop)
        stop_btn.pack(pady=10)

        # Start UDP listener
        self.listener = UDPListener(self.port)
        for config in self.strips_config:
            strip_id = config['id']
            self.listener.add_handler(strip_id, lambda p, sid=strip_id: self._on_packet(sid, p))
        self.listener.start()

        # Handle window close
        self.root.protocol("WM_DELETE_WINDOW", self._on_stop)

        # Run main loop
        self.root.mainloop()

    def stop(self):
        """Stop the emulator."""
        if self.listener:
            self.listener.stop()
        if self.root:
            self.root.quit()
            self.root.destroy()


def main():
    """Main entry point."""
    import argparse

    parser = argparse.ArgumentParser(description='LED Strip Emulator')
    parser.add_argument('--port', type=int, default=4210,
                        help='UDP port to listen on (default: 4210)')
    parser.add_argument('--strip', action='append', nargs=2, metavar=('ID', 'NUM_LEDS'),
                        help='Add a strip with given ID and LED count (can be repeated)')
    args = parser.parse_args()

    # Default strips if none specified
    if args.strip:
        strips = [{'id': int(s[0]), 'num_leds': int(s[1])} for s in args.strip]
    else:
        # Default: match the server's strip configuration
        strips = [
            {'id': 1, 'num_leds': 67},
            {'id': 2, 'num_leds': 83},
        ]

    print(f"Starting LED Strip Emulator")
    print(f"  Port: {args.port}")
    print(f"  Strips: {strips}")
    print()

    emulator = LEDStripEmulator(strips, port=args.port)
    emulator.start()


if __name__ == '__main__':
    main()
