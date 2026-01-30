#!/usr/bin/env python3
"""
LED Strip Emulator - GPU-accelerated visual simulator for LED strip output.

This program listens for UDP packets from the LED controller server
and displays the LED colors with bloom and lighting effects using Arcade (OpenGL).
"""

import sys
import os
import socket
import threading
import math
from dataclasses import dataclass, field
from typing import List, Optional, Callable
from array import array

import arcade
import arcade.gl
from arcade.gui import UIManager, UISlider, UILabel, UIBoxLayout, UIAnchorLayout

# Add parent directory to path to import from tests_external
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from fake_esp32 import LEDPacket


# Bloom shader - renders bright spots with glow effect
BLOOM_VERTEX_SHADER = """
#version 330

in vec2 in_vert;
in vec2 in_uv;
out vec2 uv;

void main() {
    gl_Position = vec4(in_vert, 0.0, 1.0);
    uv = in_uv;
}
"""

BLOOM_FRAGMENT_SHADER = """
#version 330

uniform sampler2D scene_texture;
uniform float bloom_intensity;
uniform float bloom_radius;
uniform int blur_passes;

in vec2 uv;
out vec4 fragColor;

void main() {
    vec2 tex_size = textureSize(scene_texture, 0);
    vec2 texel = 1.0 / tex_size;

    vec4 color = texture(scene_texture, uv);
    vec4 bloom = vec4(0.0);
    float total_weight = 0.0;

    // Gaussian blur for bloom
    int samples = blur_passes;
    for (int x = -samples; x <= samples; x++) {
        for (int y = -samples; y <= samples; y++) {
            float dist = length(vec2(x, y));
            if (dist <= float(samples)) {
                float weight = exp(-dist * dist / (2.0 * bloom_radius * bloom_radius));
                vec2 offset = vec2(x, y) * texel * bloom_radius;
                vec4 sample_color = texture(scene_texture, uv + offset);

                // Only bloom bright areas
                float brightness = max(max(sample_color.r, sample_color.g), sample_color.b);
                if (brightness > 0.1) {
                    bloom += sample_color * weight * brightness;
                    total_weight += weight;
                }
            }
        }
    }

    if (total_weight > 0.0) {
        bloom /= total_weight;
    }

    // Combine original with bloom
    fragColor = color + bloom * bloom_intensity;
    fragColor.a = 1.0;
}
"""

# Saturation/HDR post-process shader
POSTPROCESS_VERTEX_SHADER = """
#version 330

in vec2 in_vert;
in vec2 in_uv;
out vec2 uv;

void main() {
    gl_Position = vec4(in_vert, 0.0, 1.0);
    uv = in_uv;
}
"""

POSTPROCESS_FRAGMENT_SHADER = """
#version 330

uniform sampler2D scene_texture;
uniform float saturation;
uniform float exposure;
uniform float gamma;

in vec2 uv;
out vec4 fragColor;

vec3 rgb_to_hsl(vec3 c) {
    float maxc = max(max(c.r, c.g), c.b);
    float minc = min(min(c.r, c.g), c.b);
    float l = (maxc + minc) / 2.0;

    if (maxc == minc) {
        return vec3(0.0, 0.0, l);
    }

    float d = maxc - minc;
    float s = l > 0.5 ? d / (2.0 - maxc - minc) : d / (maxc + minc);
    float h;

    if (maxc == c.r) {
        h = (c.g - c.b) / d + (c.g < c.b ? 6.0 : 0.0);
    } else if (maxc == c.g) {
        h = (c.b - c.r) / d + 2.0;
    } else {
        h = (c.r - c.g) / d + 4.0;
    }
    h /= 6.0;

    return vec3(h, s, l);
}

float hue_to_rgb(float p, float q, float t) {
    if (t < 0.0) t += 1.0;
    if (t > 1.0) t -= 1.0;
    if (t < 1.0/6.0) return p + (q - p) * 6.0 * t;
    if (t < 1.0/2.0) return q;
    if (t < 2.0/3.0) return p + (q - p) * (2.0/3.0 - t) * 6.0;
    return p;
}

vec3 hsl_to_rgb(vec3 hsl) {
    float h = hsl.x;
    float s = hsl.y;
    float l = hsl.z;

    if (s == 0.0) {
        return vec3(l);
    }

    float q = l < 0.5 ? l * (1.0 + s) : l + s - l * s;
    float p = 2.0 * l - q;

    return vec3(
        hue_to_rgb(p, q, h + 1.0/3.0),
        hue_to_rgb(p, q, h),
        hue_to_rgb(p, q, h - 1.0/3.0)
    );
}

void main() {
    vec4 color = texture(scene_texture, uv);

    // Apply exposure
    vec3 rgb = color.rgb * exposure;

    // Apply saturation boost
    vec3 hsl = rgb_to_hsl(rgb);
    hsl.y = clamp(hsl.y * saturation, 0.0, 1.0);
    rgb = hsl_to_rgb(hsl);

    // Tone mapping (simple Reinhard)
    rgb = rgb / (rgb + vec3(1.0));

    // Gamma correction
    rgb = pow(rgb, vec3(1.0 / gamma));

    fragColor = vec4(rgb, 1.0);
}
"""


@dataclass
class LEDStrip:
    """Represents an LED strip with its visual properties."""
    strip_id: int
    num_leds: int
    colors: List[tuple] = field(default_factory=list)
    brightness: int = 255
    packet_count: int = 0
    sequence: int = 0

    def __post_init__(self):
        self.colors = [(0, 0, 0)] * self.num_leds


class UDPListener:
    """Listens for UDP packets and dispatches to handlers."""

    def __init__(self, port: int, host: str = "0.0.0.0"):
        self.port = port
        self.host = host
        self._running = False
        self._thread: Optional[threading.Thread] = None
        self._socket: Optional[socket.socket] = None
        self._handlers: dict[int, Callable] = {}  # device_id -> callback
        self._lock = threading.Lock()

    def add_handler(self, device_id: int, callback: Callable[[LEDPacket], None]):
        """Register a callback for packets with a specific device ID."""
        with self._lock:
            self._handlers[device_id] = callback

    def start(self):
        """Start listening for UDP packets."""
        if self._running:
            return

        self._socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self._socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        # Set receive buffer size for lower latency
        self._socket.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 65536)
        self._socket.bind((self.host, self.port))
        self._socket.settimeout(0.1)  # Short timeout for responsive shutdown
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
                with self._lock:
                    handler = self._handlers.get(packet.device_id)
                if handler:
                    handler(packet)
            except socket.timeout:
                continue
            except Exception as e:
                if self._running:
                    print(f"Error receiving packet: {e}")


class LEDStripEmulator(arcade.Window):
    """Main GPU-accelerated LED strip emulator using Arcade."""

    # Layout constants
    LED_SIZE = 10
    LED_SPACING = 3
    STRIP_PADDING = 20
    HEADER_HEIGHT = 50
    CONTROLS_WIDTH = 280
    STATUS_HEIGHT = 25

    def __init__(self, strips_config: List[dict], port: int = 4210):
        """
        Initialize the emulator.

        Args:
            strips_config: List of dicts with 'id' and 'num_leds' keys
            port: UDP port to listen on
        """
        self.strips_config = strips_config
        self.port = port

        # Calculate window size based on strips
        max_leds = max(c['num_leds'] for c in strips_config)
        strip_width = (self.LED_SIZE + self.LED_SPACING) * max_leds + self.LED_SPACING
        content_width = strip_width + self.STRIP_PADDING * 2

        num_strips = len(strips_config)
        strip_height = self.LED_SIZE + self.LED_SPACING * 2 + self.STATUS_HEIGHT
        content_height = (
            self.HEADER_HEIGHT +
            strip_height * num_strips +
            self.STRIP_PADDING * (num_strips + 1) +
            60  # Bottom padding
        )

        window_width = content_width + self.CONTROLS_WIDTH
        window_height = max(content_height, 400)  # Minimum height for controls

        super().__init__(
            window_width,
            window_height,
            "LED Strip Emulator",
            resizable=True,
            gl_version=(3, 3),
            vsync=False  # Disable vsync for lower latency
        )

        self.set_min_size(600, 300)
        arcade.set_background_color((30, 30, 30))

        # Strip data
        self.strips: dict[int, LEDStrip] = {}
        for config in strips_config:
            strip = LEDStrip(config['id'], config['num_leds'])
            self.strips[strip.strip_id] = strip

        # Pending updates from UDP thread (thread-safe queue)
        self._pending_updates: List[tuple] = []
        self._update_lock = threading.Lock()

        # Effect parameters
        self.bloom_intensity = 1.5
        self.bloom_radius = 2.0
        self.blur_passes = 4
        self.saturation = 1.8
        self.exposure = 1.2
        self.gamma = 1.0

        # Initialize after OpenGL context is ready
        self.offscreen_buffer = None
        self.bloom_buffer = None
        self.bloom_program = None
        self.postprocess_program = None
        self.quad_geometry = None
        self.ui_manager = None
        self.listener = None

        # FPS tracking
        self.fps_text = ""
        self.frame_count = 0
        self.fps_timer = 0.0

    def setup(self):
        """Set up the emulator after window creation."""
        # Create offscreen framebuffer for rendering LEDs
        self.offscreen_buffer = self.ctx.framebuffer(
            color_attachments=[self.ctx.texture((self.width, self.height), components=4)]
        )
        self.bloom_buffer = self.ctx.framebuffer(
            color_attachments=[self.ctx.texture((self.width, self.height), components=4)]
        )

        # Create shader programs
        self.bloom_program = self.ctx.program(
            vertex_shader=BLOOM_VERTEX_SHADER,
            fragment_shader=BLOOM_FRAGMENT_SHADER,
        )
        self.postprocess_program = self.ctx.program(
            vertex_shader=POSTPROCESS_VERTEX_SHADER,
            fragment_shader=POSTPROCESS_FRAGMENT_SHADER,
        )

        # Create fullscreen quad for post-processing
        self.quad_geometry = self.ctx.geometry([
            arcade.gl.BufferDescription(
                self.ctx.buffer(data=array('f', [
                    # x, y, u, v
                    -1.0, -1.0, 0.0, 0.0,
                     1.0, -1.0, 1.0, 0.0,
                    -1.0,  1.0, 0.0, 1.0,
                     1.0,  1.0, 1.0, 1.0,
                ])),
                '2f 2f',
                ['in_vert', 'in_uv'],
            )
        ], mode=self.ctx.TRIANGLE_STRIP)

        # Set up UI
        self._setup_ui()

        # Start UDP listener
        self.listener = UDPListener(self.port)
        for config in self.strips_config:
            strip_id = config['id']
            self.listener.add_handler(strip_id, lambda p, sid=strip_id: self._on_packet(sid, p))
        self.listener.start()

    def _setup_ui(self):
        """Set up the UI controls panel."""
        self.ui_manager = UIManager()
        self.ui_manager.enable()

        # Create control panel on the right side
        panel_layout = UIBoxLayout(vertical=True, space_between=10)

        # Title
        title = UILabel(text="Effect Controls", font_size=14, bold=True)
        panel_layout.add(title)

        # Bloom Intensity slider
        bloom_label = UILabel(text=f"Bloom: {self.bloom_intensity:.1f}", font_size=11)
        panel_layout.add(bloom_label)
        bloom_slider = UISlider(value=self.bloom_intensity, min_value=0.0, max_value=4.0, width=200)
        @bloom_slider.event("on_change")
        def on_bloom_change(event):
            self.bloom_intensity = bloom_slider.value
            bloom_label.text = f"Bloom: {self.bloom_intensity:.1f}"
        panel_layout.add(bloom_slider)

        # Bloom Radius slider
        radius_label = UILabel(text=f"Glow Size: {self.bloom_radius:.1f}", font_size=11)
        panel_layout.add(radius_label)
        radius_slider = UISlider(value=self.bloom_radius, min_value=0.5, max_value=5.0, width=200)
        @radius_slider.event("on_change")
        def on_radius_change(event):
            self.bloom_radius = radius_slider.value
            radius_label.text = f"Glow Size: {self.bloom_radius:.1f}"
        panel_layout.add(radius_slider)

        # Saturation slider
        sat_label = UILabel(text=f"Saturation: {self.saturation:.1f}", font_size=11)
        panel_layout.add(sat_label)
        sat_slider = UISlider(value=self.saturation, min_value=0.5, max_value=3.0, width=200)
        @sat_slider.event("on_change")
        def on_sat_change(event):
            self.saturation = sat_slider.value
            sat_label.text = f"Saturation: {self.saturation:.1f}"
        panel_layout.add(sat_slider)

        # Exposure slider
        exp_label = UILabel(text=f"Exposure: {self.exposure:.1f}", font_size=11)
        panel_layout.add(exp_label)
        exp_slider = UISlider(value=self.exposure, min_value=0.5, max_value=3.0, width=200)
        @exp_slider.event("on_change")
        def on_exp_change(event):
            self.exposure = exp_slider.value
            exp_label.text = f"Exposure: {self.exposure:.1f}"
        panel_layout.add(exp_slider)

        # Blur passes slider
        blur_label = UILabel(text=f"Blur Quality: {self.blur_passes}", font_size=11)
        panel_layout.add(blur_label)
        blur_slider = UISlider(value=float(self.blur_passes), min_value=1.0, max_value=8.0, width=200)
        @blur_slider.event("on_change")
        def on_blur_change(event):
            self.blur_passes = int(blur_slider.value)
            blur_label.text = f"Blur Quality: {self.blur_passes}"
        panel_layout.add(blur_slider)

        # Add spacer
        spacer = UILabel(text="", font_size=5)
        panel_layout.add(spacer)

        # Port info
        port_label = UILabel(text=f"UDP Port: {self.port}", font_size=10, text_color=(136, 255, 136))
        panel_layout.add(port_label)

        # Anchor layout to position the panel
        anchor = UIAnchorLayout()
        anchor.add(
            panel_layout,
            anchor_x="right",
            anchor_y="top",
            align_x=-20,
            align_y=-20,
        )

        self.ui_manager.add(anchor)

    def _on_packet(self, strip_id: int, packet: LEDPacket):
        """Handle received packet (called from UDP thread)."""
        with self._update_lock:
            self._pending_updates.append((strip_id, packet))

    def _process_pending_updates(self):
        """Process pending updates from UDP thread."""
        with self._update_lock:
            updates = self._pending_updates
            self._pending_updates = []

        for strip_id, packet in updates:
            strip = self.strips.get(strip_id)
            if strip:
                strip.colors = list(packet.led_data)
                strip.brightness = packet.brightness
                strip.sequence = packet.sequence
                strip.packet_count += 1

    def on_update(self, delta_time: float):
        """Update game logic."""
        self._process_pending_updates()

        # FPS tracking
        self.frame_count += 1
        self.fps_timer += delta_time
        if self.fps_timer >= 1.0:
            self.fps_text = f"FPS: {self.frame_count}"
            self.frame_count = 0
            self.fps_timer = 0.0

    def _draw_led_circle(self, x: float, y: float, r: int, g: int, b: int, brightness: int):
        """Draw a single LED with glow effect."""
        # Apply brightness
        factor = brightness / 255.0
        r = int(r * factor)
        g = int(g * factor)
        b = int(b * factor)

        # Draw outer glow (larger, dimmer circles)
        max_val = max(r, g, b)
        if max_val > 10:
            # Glow layers
            for i in range(3, 0, -1):
                glow_factor = 0.15 * (4 - i) / 3
                glow_r = int(min(255, r * glow_factor))
                glow_g = int(min(255, g * glow_factor))
                glow_b = int(min(255, b * glow_factor))
                glow_size = self.LED_SIZE / 2 + i * 2
                arcade.draw_circle_filled(x, y, glow_size, (glow_r, glow_g, glow_b, 100))

        # Draw main LED
        arcade.draw_circle_filled(x, y, self.LED_SIZE / 2, (r, g, b))

        # Draw bright center highlight for lit LEDs
        if max_val > 50:
            highlight_r = min(255, r + 100)
            highlight_g = min(255, g + 100)
            highlight_b = min(255, b + 100)
            arcade.draw_circle_filled(x, y, self.LED_SIZE / 4, (highlight_r, highlight_g, highlight_b))

    def _draw_scene_to_buffer(self):
        """Draw the LED scene to the offscreen buffer."""
        self.offscreen_buffer.use()
        self.offscreen_buffer.clear((30, 30, 30, 255))

        # Calculate starting position
        content_width = self.width - self.CONTROLS_WIDTH
        y = self.height - self.HEADER_HEIGHT - self.STRIP_PADDING

        # Draw header (in the buffer, but will be rendered to screen after post-processing)
        arcade.draw_text(
            "LED Strip Emulator",
            content_width / 2,
            self.height - 30,
            arcade.color.WHITE,
            font_size=18,
            anchor_x="center",
            bold=True,
        )

        # Draw each strip
        for config in self.strips_config:
            strip_id = config['id']
            strip = self.strips[strip_id]

            # Strip label
            label_y = y - 5
            arcade.draw_text(
                f"Strip {strip_id} ({strip.num_leds} LEDs)",
                self.STRIP_PADDING,
                label_y,
                arcade.color.WHITE,
                font_size=11,
            )

            # Draw LEDs
            led_y = y - 25
            for i, (r, g, b) in enumerate(strip.colors[:strip.num_leds]):
                led_x = self.STRIP_PADDING + self.LED_SPACING + i * (self.LED_SIZE + self.LED_SPACING) + self.LED_SIZE / 2
                self._draw_led_circle(led_x, led_y, r, g, b, strip.brightness)

            # Status text
            status_y = led_y - 25
            status_color = (136, 255, 136)  # Green like original
            arcade.draw_text(
                f"seq={strip.sequence}, brightness={strip.brightness}, packets={strip.packet_count}",
                self.STRIP_PADDING,
                status_y,
                status_color,
                font_size=10,
            )

            # Move down for next strip
            y -= self.LED_SIZE + self.LED_SPACING * 2 + self.STATUS_HEIGHT + self.STRIP_PADDING + 20

        # Draw FPS
        arcade.draw_text(
            self.fps_text,
            10,
            10,
            (100, 100, 100),
            font_size=10,
        )

    def on_draw(self):
        """Render the screen with post-processing effects."""
        # Step 1: Draw scene to offscreen buffer
        self._draw_scene_to_buffer()

        # Step 2: Apply bloom effect
        self.bloom_buffer.use()
        self.bloom_buffer.clear()
        self.offscreen_buffer.color_attachments[0].use(0)
        self.bloom_program['scene_texture'] = 0
        self.bloom_program['bloom_intensity'] = self.bloom_intensity
        self.bloom_program['bloom_radius'] = self.bloom_radius
        self.bloom_program['blur_passes'] = self.blur_passes
        self.quad_geometry.render(self.bloom_program)

        # Step 3: Apply final post-processing (saturation, exposure, gamma)
        self.use()  # Switch to default framebuffer
        self.clear()
        self.bloom_buffer.color_attachments[0].use(0)
        self.postprocess_program['scene_texture'] = 0
        self.postprocess_program['saturation'] = self.saturation
        self.postprocess_program['exposure'] = self.exposure
        self.postprocess_program['gamma'] = self.gamma
        self.quad_geometry.render(self.postprocess_program)

        # Draw UI on top (no post-processing)
        self.ui_manager.draw()

    def on_resize(self, width: int, height: int):
        """Handle window resize."""
        super().on_resize(width, height)

        # Recreate framebuffers at new size
        if self.offscreen_buffer:
            self.offscreen_buffer = self.ctx.framebuffer(
                color_attachments=[self.ctx.texture((width, height), components=4)]
            )
        if self.bloom_buffer:
            self.bloom_buffer = self.ctx.framebuffer(
                color_attachments=[self.ctx.texture((width, height), components=4)]
            )

    def on_key_press(self, key, modifiers):
        """Handle key presses."""
        if key == arcade.key.ESCAPE:
            self.close()

    def on_close(self):
        """Handle window close."""
        if self.listener:
            self.listener.stop()
        super().on_close()


def main():
    """Main entry point."""
    import argparse

    parser = argparse.ArgumentParser(description='LED Strip Emulator (GPU Accelerated)')
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

    print(f"Starting LED Strip Emulator (GPU Accelerated)")
    print(f"  Port: {args.port}")
    print(f"  Strips: {strips}")
    print(f"  Press ESC to exit")
    print()

    emulator = LEDStripEmulator(strips, port=args.port)
    emulator.setup()
    arcade.run()


if __name__ == '__main__':
    main()
