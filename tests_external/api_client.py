"""
REST API client for the LED controller.

This module provides a simple client for interacting with the LED controller
API via HTTP requests. It's completely independent of the server code.
"""

import requests
from typing import Dict, List, Any, Optional


class LEDControllerAPI:
    """Client for the LED controller REST API."""

    def __init__(self, base_url: str = "http://localhost:5000"):
        self.base_url = base_url.rstrip("/")
        self.session = requests.Session()

    def _get(self, endpoint: str) -> Dict[str, Any]:
        """Make a GET request."""
        response = self.session.get(f"{self.base_url}{endpoint}")
        response.raise_for_status()
        return response.json()

    def _post(self, endpoint: str, data: Dict[str, Any]) -> Dict[str, Any]:
        """Make a POST request."""
        response = self.session.post(f"{self.base_url}{endpoint}", json=data)
        response.raise_for_status()
        return response.json()

    # --- Global endpoints (apply to ALL strips) ---

    # Mode endpoints
    def get_modes(self) -> List[str]:
        """Get list of available modes."""
        response = self.session.get(f"{self.base_url}/api/modes")
        response.raise_for_status()
        return response.json()

    def get_current_mode(self) -> str:
        """Get the current mode."""
        return self._get("/api/modes/current")["mode"]

    def set_mode(self, mode: str) -> str:
        """Set the current mode."""
        return self._post("/api/modes/current", {"mode": mode})["mode"]

    # Brightness endpoints
    def get_brightness(self) -> int:
        """Get current brightness."""
        return self._get("/api/brightness")["brightness"]

    def set_brightness(self, value: int) -> int:
        """Set brightness (0-255)."""
        return self._post("/api/brightness", {"brightness": value})["brightness"]

    # Delay endpoints
    def get_delay(self) -> float:
        """Get current delay."""
        return self._get("/api/delay")["delay"]

    def set_delay(self, value: float) -> float:
        """Set delay in seconds."""
        return self._post("/api/delay", {"delay": value})["delay"]

    # Options endpoints
    def get_opts(self) -> Dict[str, Any]:
        """Get current mode options."""
        return self._get("/api/opts")["opts"]

    def set_opts(self, opts: Dict[str, Any]) -> Dict[str, Any]:
        """Set mode options."""
        return self._post("/api/opts", opts)["opts"]

    # Strip info
    def get_strips(self) -> List[Dict[str, Any]]:
        """Get information about configured LED strips."""
        return self._get("/api/strips")["strips"]

    def configure_strip(
        self, strip_id: int, hostname: Optional[str] = None, port: Optional[int] = None
    ) -> Dict[str, Any]:
        """Configure a strip's network settings (debug mode only).

        This allows tests to redirect UDP packets to localhost listeners.
        """
        data = {}
        if hostname is not None:
            data["hostname"] = hostname
        if port is not None:
            data["port"] = port
        return self._post(f"/api/strips/{strip_id}", data)

    # --- Per-strip endpoints ---

    def get_strip_mode(self, strip_id: int) -> str:
        """Get mode for a specific strip."""
        return self._get(f"/api/strips/{strip_id}/mode")["mode"]

    def set_strip_mode(self, strip_id: int, mode: str) -> str:
        """Set mode for a specific strip."""
        return self._post(f"/api/strips/{strip_id}/mode", {"mode": mode})["mode"]

    def get_strip_brightness(self, strip_id: int) -> int:
        """Get brightness for a specific strip."""
        return self._get(f"/api/strips/{strip_id}/brightness")["brightness"]

    def set_strip_brightness(self, strip_id: int, value: int) -> int:
        """Set brightness for a specific strip (0-255)."""
        return self._post(f"/api/strips/{strip_id}/brightness", {"brightness": value})["brightness"]

    def get_strip_delay(self, strip_id: int) -> float:
        """Get delay for a specific strip."""
        return self._get(f"/api/strips/{strip_id}/delay")["delay"]

    def set_strip_delay(self, strip_id: int, value: float) -> float:
        """Set delay for a specific strip in seconds."""
        return self._post(f"/api/strips/{strip_id}/delay", {"delay": value})["delay"]

    def get_strip_opts(self, strip_id: int) -> Dict[str, Any]:
        """Get options for a specific strip."""
        return self._get(f"/api/strips/{strip_id}/opts")["opts"]

    def set_strip_opts(self, strip_id: int, opts: Dict[str, Any]) -> Dict[str, Any]:
        """Set options for a specific strip."""
        return self._post(f"/api/strips/{strip_id}/opts", opts)["opts"]

    # Looper control (debug mode only)
    def get_looper_state(self) -> Dict[str, Any]:
        """Get current looper state."""
        return self._get("/api/looper")

    def control_looper(
        self, iterations: Optional[int] = None, next_state: Optional[str] = None
    ) -> Dict[str, Any]:
        """Control the looper.

        Args:
            iterations: Number of iterations to run (then pause)
            next_state: "pause" or "running"

        Returns:
            Current looper state after applying changes
        """
        data = {}
        if iterations is not None:
            data["iterations"] = iterations
        if next_state is not None:
            data["next_state"] = next_state
        return self._post("/api/looper", data)

    def pause_looper(self) -> Dict[str, Any]:
        """Pause the looper."""
        return self.control_looper(next_state="pause")

    def resume_looper(self) -> Dict[str, Any]:
        """Resume the looper."""
        return self.control_looper(next_state="running")

    def step_looper(self, iterations: int = 1) -> Dict[str, Any]:
        """Run the looper for a specific number of iterations then pause."""
        return self.control_looper(iterations=iterations)

    def is_debug_mode(self) -> bool:
        """Check if server is running in debug mode."""
        state = self.get_looper_state()
        return state.get("debug_mode", False)
