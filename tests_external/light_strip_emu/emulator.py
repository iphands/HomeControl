#!/usr/bin/env python3
"""
LED Strip Emulator - GPU-accelerated visual simulator for LED strip output.

This program listens for UDP packets from the LED controller server
and displays the LED colors with bloom and lighting effects using Arcade (OpenGL).

Optimized for low latency with efficient two-pass Gaussian blur bloom.
"""

import sys
import os
import socket
import threading
from dataclasses import dataclass, field
from typing import List, Optional, Callable, Dict
from array import array

# Add parent directory to path to import from tests_external
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from fake_esp32 import LEDPacket

try:
    import arcade
    import arcade.gl
    from arcade.gui import UIManager, UISlider, UILabel, UIBoxLayout, UIAnchorLayout, UIHBoxLayout
except ImportError:
    print("Error: arcade library not installed.")
    print("Install with: pip install arcade>=3.3.3")
    sys.exit(1)


# Vertex shader for all post-processing
VERTEX_SHADER = """
#version 330

in vec2 in_vert;
in vec2 in_uv;
out vec2 uv;

void main() {
    gl_Position = vec4(in_vert, 0.0, 1.0);
    uv = in_uv;
}
"""

# Horizontal blur shader (first pass of bloom)
BLUR_HORIZONTAL_FRAGMENT = """
#version 330

uniform sampler2D scene_texture;
uniform float bloom_radius;
uniform float bloom_threshold;

in vec2 uv;
out vec4 fragColor;

void main() {
    vec2 tex_size = textureSize(scene_texture, 0);
    vec2 texel = 1.0 / tex_size;
    
    // 9-tap Gaussian blur with predefined weights
    float weights[5] = float[](0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216);
    
    vec4 result = texture(scene_texture, uv) * weights[0];
    float brightness = max(max(result.r, result.g), result.b);
    
    // Only blur bright areas
    if (brightness > bloom_threshold) {
        for (int i = 1; i < 5; i++) {
            vec2 offset = vec2(texel.x * bloom_radius * float(i), 0.0);
            vec4 sample_right = texture(scene_texture, uv + offset);
            vec4 sample_left = texture(scene_texture, uv - offset);
            
            // Only sample bright pixels
            float br_right = max(max(sample_right.r, sample_right.g), sample_right.b);
            float br_left = max(max(sample_left.r, sample_left.g), sample_left.b);
            
            if (br_right > bloom_threshold) {
                result += sample_right * weights[i];
            }
            if (br_left > bloom_threshold) {
                result += sample_left * weights[i];
            }
        }
    }
    
    fragColor = result;
}
"""

# Vertical blur shader (second pass of bloom)
BLUR_VERTICAL_FRAGMENT = """
#version 330

uniform sampler2D scene_texture;
uniform sampler2D horizontal_blur;
uniform float bloom_radius;
uniform float bloom_intensity;
uniform float bloom_threshold;

in vec2 uv;
out vec4 fragColor;

void main() {
    vec2 tex_size = textureSize(horizontal_blur, 0);
    vec2 texel = 1.0 / tex_size;
    
    // 9-tap Gaussian blur with predefined weights
    float weights[5] = float[](0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216);
    
    vec4 blur_result = texture(horizontal_blur, uv) * weights[0];
    
    for (int i = 1; i < 5; i++) {
        vec2 offset = vec2(0.0, texel.y * bloom_radius * float(i));
        vec4 sample_up = texture(horizontal_blur, uv + offset);
        vec4 sample_down = texture(horizontal_blur, uv - offset);
        blur_result += sample_up * weights[i];
        blur_result += sample_down * weights[i];
    }
    
    vec4 original = texture(scene_texture, uv);
    
    // Combine original with bloom
    fragColor = original + blur_result * bloom_intensity;
}
"""

# Saturation post-process shader
POSTPROCESS_FRAGMENT = """
#version 330

uniform sampler2D scene_texture;
uniform float saturation;

in vec2 uv;
out vec4 fragColor;

vec3 rgb_to_hsv(vec3 c) {
    vec4 K = vec4(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    vec4 p = mix(vec4(c.bg, K.wz), vec4(c.gb, K.xy), step(c.b, c.g));
    vec4 q = mix(vec4(p.xyw, c.r), vec4(c.r, p.yzx), step(p.x, c.r));
    
    float d = q.x - min(q.w, q.y);
    float e = 1.0e-10;
    return vec3(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);
}

vec3 hsv_to_rgb(vec3 c) {
    vec4 K = vec4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    vec3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);
}

void main() {
    vec4 color = texture(scene_texture, uv);
    
    // Apply saturation boost using HSV
    vec3 hsv = rgb_to_hsv(color.rgb);
    hsv.y = clamp(hsv.y * saturation, 0.0, 1.0);
    vec3 rgb = hsv_to_rgb(hsv);
    
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
        self._handlers: dict[int, Callable] = {}
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
        self._socket.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 262144)
        # Enable non-blocking mode for better responsiveness
        self._socket.setblocking(False)
        self._socket.bind((self.host, self.port))
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
        import select
        while self._running:
            try:
                # Use select for non-blocking socket with timeout
                ready, _, _ = select.select([self._socket], [], [], 0.05)
                if ready:
                    data, addr = self._socket.recvfrom(2048)
                    packet = LEDPacket.from_bytes(data)
                    with self._lock:
                        handler = self._handlers.get(packet.device_id)
                    if handler:
                        handler(packet)
            except (socket.error, select.error):
                continue
            except Exception as e:
                if self._running:
                    print(f"Error receiving packet: {e}")


class LEDStripEmulator(arcade.Window):
    """Main GPU-accelerated LED strip emulator using Arcade."""

    # Layout constants - controls under strips layout
    LED_SIZE = 4  # Smaller LEDs for more compact display
    LED_SPACING = 2  # 2px spacing between LEDs
    STRIP_PADDING = 20
    HEADER_HEIGHT = 50
    CONTROLS_HEIGHT = 200  # Height for controls panel underneath
    SECTION_PADDING = 15  # Padding inside each strip section border
    STATUS_HEIGHT = 25
    
    # Colors matching tkinter version
    BG_COLOR = (45, 45, 45)  # #2d2d2d
    CANVAS_BG_COLOR = (26, 26, 26)  # #1a1a1a
    TEXT_COLOR = arcade.color.WHITE
    STATUS_COLOR = (136, 255, 136)  # #88ff88
    LED_OUTLINE_COLOR = (51, 51, 51)  # #333333
    SECTION_BORDER_COLOR = (80, 80, 80)  # #505050 - border around each strip section
    SECTION_BG_COLOR = (35, 35, 35)  # #232323 - background inside strip sections

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
        # Full width for strips with padding on both sides
        content_width = strip_width + self.STRIP_PADDING * 2
        window_width = max(content_width, 800)  # Minimum width for controls

        num_strips = len(strips_config)
        # Each strip section has border, label, LEDs, status, and padding
        strip_section_height = (
            self.SECTION_PADDING +  # Top padding inside border
            20 +  # Label height
            self.LED_SIZE + self.LED_SPACING * 2 +  # LEDs
            self.STATUS_HEIGHT +  # Status text
            self.SECTION_PADDING  # Bottom padding inside border
        )
        strips_total_height = (
            self.HEADER_HEIGHT +
            strip_section_height * num_strips +
            self.STRIP_PADDING * (num_strips + 1)
        )
        # Total height includes strips and controls panel underneath
        window_height = strips_total_height + self.CONTROLS_HEIGHT

        super().__init__(
            window_width,
            window_height,
            "LED Strip Emulator (GPU Accelerated)",
            resizable=True,
            gl_version=(3, 3),
            vsync=False,  # Disable vsync for lower latency
            antialiasing=True
        )

        self.set_minimum_size(600, 300)
        arcade.set_background_color(self.BG_COLOR)

        # Strip data
        self.strips: dict[int, LEDStrip] = {}
        for config in strips_config:
            strip = LEDStrip(config['id'], config['num_leds'])
            self.strips[strip.strip_id] = strip

        # Pending updates from UDP thread (thread-safe queue)
        self._pending_updates: List[tuple] = []
        self._update_lock = threading.Lock()

        # Effect parameters (only affect LEDs, not UI)
        self.bloom_intensity = 1.5
        self.bloom_radius = 2.0
        self.bloom_threshold = 0.1
        self.saturation = 1.8
        self.led_glow_size = 2.0

        # Initialize after OpenGL context is ready
        self.scene_buffer = None
        self.blur_h_buffer = None
        self.blur_v_buffer = None
        self.blur_h_program = None
        self.blur_v_program = None
        self.postprocess_program = None
        self.quad_geometry = None
        self.ui_manager = None
        self.listener = None

        # Text objects for efficient rendering
        self.header_text: Optional[arcade.Text] = None
        self.fps_text_obj: Optional[arcade.Text] = None
        self.strip_labels: Dict[int, arcade.Text] = {}
        self.strip_status: Dict[int, arcade.Text] = {}

        # FPS tracking
        self.frame_count = 0
        self.fps_timer = 0.0
        self.last_packet_time = 0.0
        self.packets_per_second = 0
        self.packet_counter = 0
        self.packet_timer = 0.0

    def setup(self):
        """Set up the emulator after window creation."""
        # Create offscreen framebuffers
        self._create_framebuffers()

        # Create shader programs
        self.blur_h_program = self.ctx.program(
            vertex_shader=VERTEX_SHADER,
            fragment_shader=BLUR_HORIZONTAL_FRAGMENT,
        )
        self.blur_v_program = self.ctx.program(
            vertex_shader=VERTEX_SHADER,
            fragment_shader=BLUR_VERTICAL_FRAGMENT,
        )
        self.postprocess_program = self.ctx.program(
            vertex_shader=VERTEX_SHADER,
            fragment_shader=POSTPROCESS_FRAGMENT,
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

        # Create text objects
        self._create_text_objects()

        # Set up UI
        self._setup_ui()

        # Start UDP listener
        self.listener = UDPListener(self.port)
        for config in self.strips_config:
            strip_id = config['id']
            self.listener.add_handler(strip_id, lambda p, sid=strip_id: self._on_packet(sid, p))
        self.listener.start()

    def _create_framebuffers(self):
        """Create framebuffers for offscreen rendering."""
        width, height = self.width, self.height
        self.scene_buffer = self.ctx.framebuffer(
            color_attachments=[self.ctx.texture((width, height), components=4)]
        )
        self.blur_h_buffer = self.ctx.framebuffer(
            color_attachments=[self.ctx.texture((width, height), components=4)]
        )
        self.blur_v_buffer = self.ctx.framebuffer(
            color_attachments=[self.ctx.texture((width, height), components=4)]
        )

    def _create_text_objects(self):
        """Create Text objects for efficient rendering."""
        # Header text centered at top
        self.header_text = arcade.Text(
            "LED Strip Emulator",
            self.width / 2,
            self.height - 30,
            self.TEXT_COLOR,
            font_size=16,
            anchor_x="center",
            bold=True,
        )

        # Port info centered under header
        self.port_text = arcade.Text(
            f"Listening on UDP port {self.port}",
            self.width / 2,
            self.height - 50,
            self.TEXT_COLOR,
            font_size=11,
            anchor_x="center",
        )

        # FPS text at bottom left (above controls)
        self.fps_text_obj = arcade.Text(
            "",
            10,
            self.CONTROLS_HEIGHT - 30,
            (150, 150, 150),
            font_size=10,
        )

        # Create text objects for each strip within bordered sections
        section_height = (
            self.SECTION_PADDING +
            20 +  # Label
            self.LED_SIZE + self.LED_SPACING * 2 +  # LEDs
            self.STATUS_HEIGHT +
            self.SECTION_PADDING
        )
        y = self.height - self.HEADER_HEIGHT - self.STRIP_PADDING

        for config in self.strips_config:
            strip_id = config['id']
            strip = self.strips[strip_id]

            # Section starts at y, label is at top inside border
            section_top = y
            label_y = section_top - self.SECTION_PADDING - 12

            # Strip label (white text like tkinter)
            self.strip_labels[strip_id] = arcade.Text(
                f"Strip {strip_id} ({strip.num_leds} LEDs)",
                self.STRIP_PADDING + self.SECTION_PADDING + 5,
                label_y,
                self.TEXT_COLOR,
                font_size=11,
            )

            # Status text at bottom of section
            status_y = section_top - section_height + self.SECTION_PADDING + 12
            self.strip_status[strip_id] = arcade.Text(
                f"Strip {strip_id}: waiting for packets...",
                self.STRIP_PADDING + self.SECTION_PADDING + 5,
                status_y,
                self.STATUS_COLOR,
                font_size=10,
            )

            # Move down for next strip
            y -= section_height + self.STRIP_PADDING

    def _setup_ui(self):
        """Set up the UI controls panel with two columns underneath strips."""
        self.ui_manager = UIManager()
        self.ui_manager.enable()

        # Create dark theme slider style
        dark_slider_style = {
            "normal": UISlider.UIStyle(
                bg=(60, 60, 60),
                border=(100, 100, 100),
                border_width=1,
                filled_track=(0, 150, 200),
                filled_step=None,
                unfilled_track=(40, 40, 40),
                unfilled_step=None,
            ),
            "hover": UISlider.UIStyle(
                bg=(70, 70, 70),
                border=(0, 180, 230),
                border_width=2,
                filled_track=(0, 180, 230),
                filled_step=None,
                unfilled_track=(50, 50, 50),
                unfilled_step=None,
            ),
            "press": UISlider.UIStyle(
                bg=(0, 150, 200),
                border=(0, 200, 255),
                border_width=2,
                filled_track=(0, 200, 255),
                filled_step=None,
                unfilled_track=(60, 60, 60),
                unfilled_step=None,
            ),
            "disabled": UISlider.UIStyle(
                bg=(40, 40, 40),
                border=(60, 60, 60),
                border_width=1,
                filled_track=(80, 80, 80),
                unfilled_track=(40, 40, 40),
                filled_step=None,
                unfilled_step=None,
            ),
        }

        # Main controls container at the bottom
        controls_layout = UIBoxLayout(vertical=True, space_between=5)

        # Title with white text
        title = UILabel(
            text="Effect Controls",
            font_size=14,
            bold=True,
            text_color=self.TEXT_COLOR,
        )
        controls_layout.add(title)

        # Two-column layout for controls
        columns_layout = UIHBoxLayout(space_between=40)

        # Left column - Effect sliders
        left_column = UIBoxLayout(vertical=True, space_between=8)

        # Bloom Intensity slider
        bloom_label = UILabel(
            text=f"Bloom Intensity: {self.bloom_intensity:.1f}",
            font_size=10,
            text_color=self.TEXT_COLOR,
        )
        left_column.add(bloom_label)
        bloom_slider = UISlider(
            value=self.bloom_intensity,
            min_value=0.0,
            max_value=3.0,
            width=300,
            style=dark_slider_style,
        )
        @bloom_slider.event("on_change")
        def on_bloom_change(event):
            self.bloom_intensity = bloom_slider.value
            bloom_label.text = f"Bloom Intensity: {self.bloom_intensity:.1f}"
        left_column.add(bloom_slider)

        # Bloom Radius slider
        radius_label = UILabel(
            text=f"Glow Radius: {self.bloom_radius:.1f}",
            font_size=10,
            text_color=self.TEXT_COLOR,
        )
        left_column.add(radius_label)
        radius_slider = UISlider(
            value=self.bloom_radius,
            min_value=0.5,
            max_value=6.0,
            width=300,
            style=dark_slider_style,
        )
        @radius_slider.event("on_change")
        def on_radius_change(event):
            self.bloom_radius = radius_slider.value
            radius_label.text = f"Glow Radius: {self.bloom_radius:.1f}"
        left_column.add(radius_slider)

        # Bloom Threshold slider
        threshold_label = UILabel(
            text=f"Bloom Threshold: {self.bloom_threshold:.2f}",
            font_size=10,
            text_color=self.TEXT_COLOR,
        )
        left_column.add(threshold_label)
        threshold_slider = UISlider(
            value=self.bloom_threshold,
            min_value=0.0,
            max_value=0.5,
            width=300,
            style=dark_slider_style,
        )
        @threshold_slider.event("on_change")
        def on_threshold_change(event):
            self.bloom_threshold = threshold_slider.value
            threshold_label.text = f"Bloom Threshold: {self.bloom_threshold:.2f}"
        left_column.add(threshold_slider)

        columns_layout.add(left_column)

        # Right column - More controls and info
        right_column = UIBoxLayout(vertical=True, space_between=8)

        # Saturation slider
        sat_label = UILabel(
            text=f"Saturation: {self.saturation:.1f}",
            font_size=10,
            text_color=self.TEXT_COLOR,
        )
        right_column.add(sat_label)
        sat_slider = UISlider(
            value=self.saturation,
            min_value=0.5,
            max_value=3.0,
            width=300,
            style=dark_slider_style,
        )
        @sat_slider.event("on_change")
        def on_sat_change(event):
            self.saturation = sat_slider.value
            sat_label.text = f"Saturation: {self.saturation:.1f}"
        right_column.add(sat_slider)

        # LED Glow Size slider
        glow_label = UILabel(
            text=f"LED Glow: {self.led_glow_size:.1f}",
            font_size=10,
            text_color=self.TEXT_COLOR,
        )
        right_column.add(glow_label)
        glow_slider = UISlider(
            value=self.led_glow_size,
            min_value=0.0,
            max_value=5.0,
            width=300,
            style=dark_slider_style,
        )
        @glow_slider.event("on_change")
        def on_glow_change(event):
            self.led_glow_size = glow_slider.value
            glow_label.text = f"LED Glow: {self.led_glow_size:.1f}"
        right_column.add(glow_slider)

        # Info section
        info_layout = UIHBoxLayout(space_between=30)

        port_label = UILabel(
            text=f"UDP Port: {self.port}",
            font_size=10,
            text_color=self.STATUS_COLOR,
        )
        info_layout.add(port_label)

        self.pps_label = UILabel(
            text="Packets/sec: 0",
            font_size=10,
            text_color=(100, 200, 255),
        )
        info_layout.add(self.pps_label)

        gpu_label = UILabel(
            text="GPU: Enabled",
            font_size=10,
            text_color=(255, 200, 100),
        )
        info_layout.add(gpu_label)

        right_column.add(info_layout)

        columns_layout.add(right_column)
        controls_layout.add(columns_layout)

        # Anchor layout to position controls at the bottom
        anchor = UIAnchorLayout()
        anchor.add(
            controls_layout,
            anchor_x="center",
            anchor_y="bottom",
            align_y=20,
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
                self.packet_counter += 1
                
                # Update status text (matching tkinter format)
                if strip_id in self.strip_status:
                    self.strip_status[strip_id].text = (
                        f"Strip {strip_id}: seq={strip.sequence}, brightness={strip.brightness}, packets={strip.packet_count}"
                    )

    def on_update(self, delta_time: float):
        """Update game logic."""
        self._process_pending_updates()

        # FPS tracking
        self.frame_count += 1
        self.fps_timer += delta_time
        if self.fps_timer >= 1.0:
            fps_text = f"FPS: {self.frame_count}"
            if self.fps_text_obj:
                self.fps_text_obj.text = fps_text
            self.frame_count = 0
            self.fps_timer = 0.0

        # Packets per second tracking
        self.packet_timer += delta_time
        if self.packet_timer >= 1.0:
            self.packets_per_second = self.packet_counter
            self.packet_counter = 0
            self.packet_timer = 0.0
            if hasattr(self, 'pps_label'):
                self.pps_label.text = f"Packets/sec: {self.packets_per_second}"

    def _draw_led_rectangle(self, x: float, y: float, r: int, g: int, b: int, brightness: int):
        """Draw a single LED rectangle with glow effect (matching old tkinter version)."""
        # Apply brightness
        factor = brightness / 255.0
        r = int(r * factor)
        g = int(g * factor)
        b = int(b * factor)

        max_val = max(r, g, b)
        if max_val > 5:
            # Draw outer glow layers for bloom effect
            glow_layers = int(self.led_glow_size)
            for i in range(glow_layers, 0, -1):
                glow_factor = 0.2 * (glow_layers - i + 1) / glow_layers
                glow_r = int(min(255, r * glow_factor))
                glow_g = int(min(255, g * glow_factor))
                glow_b = int(min(255, b * glow_factor))
                glow_size = self.LED_SIZE + i * 2.5
                alpha = int(80 * glow_factor)
                glow_left = x - glow_size / 2
                glow_bottom = y - glow_size / 2
                arcade.draw_lbwh_rectangle_filled(glow_left, glow_bottom, glow_size, glow_size, (glow_r, glow_g, glow_b, alpha))

            # Draw main LED rectangle (square like old version)
            led_left = x - self.LED_SIZE / 2
            led_bottom = y - self.LED_SIZE / 2
            arcade.draw_lbwh_rectangle_filled(led_left, led_bottom, self.LED_SIZE, self.LED_SIZE, (r, g, b))

            # Draw bright center highlight for extra shine
            if max_val > 30:
                highlight_factor = 1.3
                highlight_r = min(255, int(r * highlight_factor))
                highlight_g = min(255, int(g * highlight_factor))
                highlight_b = min(255, int(b * highlight_factor))
                highlight_size = self.LED_SIZE * 0.6
                highlight_left = x - highlight_size / 2
                highlight_bottom = y - highlight_size / 2
                arcade.draw_lbwh_rectangle_filled(highlight_left, highlight_bottom, highlight_size, highlight_size, (highlight_r, highlight_g, highlight_b))

            # Draw very bright core for intense LEDs
            if max_val > 100:
                core_r = min(255, r + 80)
                core_g = min(255, g + 80)
                core_b = min(255, b + 80)
                core_size = self.LED_SIZE * 0.3
                core_left = x - core_size / 2
                core_bottom = y - core_size / 2
                arcade.draw_lbwh_rectangle_filled(core_left, core_bottom, core_size, core_size, (core_r, core_g, core_b))
        else:
            # Draw dim/off LED rectangle
            led_left = x - self.LED_SIZE / 2
            led_bottom = y - self.LED_SIZE / 2
            arcade.draw_lbwh_rectangle_filled(led_left, led_bottom, self.LED_SIZE, self.LED_SIZE, (r, g, b))

    def _draw_leds_to_buffer(self):
        """Draw LEDs and strip section borders to the offscreen buffer."""
        self.scene_buffer.use()
        self.scene_buffer.clear()
        # Draw canvas background (matching tkinter #1a1a1a)
        arcade.draw_lbwh_rectangle_filled(0, 0, self.width, self.height, self.CANVAS_BG_COLOR)

        # Calculate section dimensions
        section_height = (
            self.SECTION_PADDING +
            20 +  # Label
            self.LED_SIZE + self.LED_SPACING * 2 +  # LEDs
            self.STATUS_HEIGHT +
            self.SECTION_PADDING
        )
        section_width = self.width - self.STRIP_PADDING * 2

        # Calculate starting position
        y = self.height - self.HEADER_HEIGHT - self.STRIP_PADDING

        # Draw each strip section with border
        for config in self.strips_config:
            strip_id = config['id']
            strip = self.strips[strip_id]

            section_top = y
            section_bottom = y - section_height

            # Draw section background
            arcade.draw_lbwh_rectangle_filled(
                self.STRIP_PADDING,
                section_bottom,
                section_width,
                section_height,
                self.SECTION_BG_COLOR
            )

            # Draw section border (1px outline)
            arcade.draw_lbwh_rectangle_outline(
                self.STRIP_PADDING,
                section_bottom,
                section_width,
                section_height,
                self.SECTION_BORDER_COLOR,
                1
            )

            # Calculate LED Y position (centered vertically in the LED area)
            led_area_top = section_top - self.SECTION_PADDING - 20
            led_area_bottom = section_bottom + self.SECTION_PADDING + self.STATUS_HEIGHT
            led_y = (led_area_top + led_area_bottom) / 2

            # Draw LEDs
            led_start_x = self.STRIP_PADDING + self.SECTION_PADDING + self.LED_SPACING
            for i, (r, g, b) in enumerate(strip.colors[:strip.num_leds]):
                led_x = led_start_x + i * (self.LED_SIZE + self.LED_SPACING) + self.LED_SIZE / 2
                self._draw_led_rectangle(led_x, led_y, r, g, b, strip.brightness)

            # Move down for next strip
            y -= section_height + self.STRIP_PADDING

    def _draw_text_overlay(self):
        """Draw text and UI labels directly to screen (no bloom)."""
        # Draw header
        if self.header_text:
            self.header_text.draw()

        # Draw port info
        if self.port_text:
            self.port_text.draw()

        # Draw strip labels
        for strip_id, text_obj in self.strip_labels.items():
            text_obj.draw()

        # Draw strip status
        for strip_id, text_obj in self.strip_status.items():
            text_obj.draw()

        # Draw FPS
        if self.fps_text_obj:
            self.fps_text_obj.draw()

    def on_draw(self):
        """Render the screen with post-processing effects."""
        # Step 1: Draw only LEDs to offscreen buffer
        self._draw_leds_to_buffer()

        # Step 2: Horizontal blur pass
        self.blur_h_buffer.use()
        self.blur_h_buffer.clear()
        self.scene_buffer.color_attachments[0].use(0)
        self.blur_h_program['scene_texture'] = 0
        self.blur_h_program['bloom_radius'] = self.bloom_radius
        self.blur_h_program['bloom_threshold'] = self.bloom_threshold
        self.quad_geometry.render(self.blur_h_program)

        # Step 3: Vertical blur pass + combine with bloom
        self.blur_v_buffer.use()
        self.blur_v_buffer.clear()
        self.scene_buffer.color_attachments[0].use(0)
        self.blur_h_buffer.color_attachments[0].use(1)
        self.blur_v_program['scene_texture'] = 0
        self.blur_v_program['horizontal_blur'] = 1
        self.blur_v_program['bloom_radius'] = self.bloom_radius
        self.blur_v_program['bloom_intensity'] = self.bloom_intensity
        self.quad_geometry.render(self.blur_v_program)

        # Step 4: Render to screen with saturation
        self.use()
        self.clear()
        
        # Draw bloomed LEDs with saturation to entire screen
        self.blur_v_buffer.color_attachments[0].use(0)
        self.postprocess_program['scene_texture'] = 0
        self.postprocess_program['saturation'] = self.saturation
        self.quad_geometry.render(self.postprocess_program)

        # Step 5: Draw text overlay (not affected by bloom)
        self._draw_text_overlay()

        # Step 6: Draw UI controls (not affected by bloom)
        self.ui_manager.draw()

    def on_resize(self, width: int, height: int):
        """Handle window resize."""
        super().on_resize(width, height)

        # Recreate framebuffers at new size
        if self.scene_buffer:
            self._create_framebuffers()
            
        # Recreate text objects with new positions
        if self.header_text:
            self._create_text_objects()

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

    print(f"Starting LED Strip Emulator")
    print(f"  Port: {args.port}")
    print(f"  Strips: {strips}")
    print()

    emulator = LEDStripEmulator(strips, port=args.port)
    emulator.setup()
    arcade.run()


if __name__ == '__main__':
    main()
