#!/usr/bin/env python3
"""
Test runner for the LED controller functional tests.

This script can optionally start the server in debug mode before running tests.

Usage:
    # Run tests (assumes server is already running)
    python run_tests.py

    # Start server and run tests
    python run_tests.py --start-server

    # Run specific test
    python run_tests.py -k test_get_modes
"""

import argparse
import subprocess
import sys
import time
import os
import signal
import requests


def wait_for_server(url: str, timeout: float = 30.0) -> bool:
    """Wait for the server to become available."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            response = requests.get(f"{url}/looper", timeout=2.0)
            if response.status_code == 200:
                data = response.json()
                if data.get("debug_mode"):
                    print(f"Server is ready (debug mode enabled)")
                    return True
                else:
                    print("Warning: Server running but debug mode not enabled")
                    return False
        except requests.exceptions.RequestException:
            pass
        time.sleep(0.5)
        print(".", end="", flush=True)
    print("\nTimeout waiting for server")
    return False


def main():
    parser = argparse.ArgumentParser(description="Run LED controller functional tests")
    parser.add_argument(
        "--start-server",
        action="store_true",
        help="Start the server before running tests",
    )
    parser.add_argument(
        "--server-url",
        default="http://localhost:5000",
        help="Server URL (default: http://localhost:5000)",
    )
    parser.add_argument(
        "-k",
        dest="test_filter",
        help="Only run tests matching the given expression",
    )
    parser.add_argument(
        "-v", "--verbose",
        action="store_true",
        help="Verbose output",
    )

    args, extra_args = parser.parse_known_args()

    server_process = None
    tests_dir = os.path.dirname(os.path.abspath(__file__))
    app_dir = os.path.join(os.path.dirname(tests_dir), "app")

    try:
        if args.start_server:
            print("Starting server in debug mode...")
            # Start server as subprocess
            server_process = subprocess.Popen(
                [sys.executable, "-m", "__init__", "--debug"],
                cwd=app_dir,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
            )
            print(f"Server PID: {server_process.pid}")

            # Wait for server to be ready
            print("Waiting for server to start", end="")
            if not wait_for_server(args.server_url):
                print("Failed to start server")
                return 1

        # Build pytest command
        pytest_args = [sys.executable, "-m", "pytest"]
        pytest_args.append(os.path.join(tests_dir, "test_led_controller.py"))

        if args.verbose:
            pytest_args.append("-v")

        if args.test_filter:
            pytest_args.extend(["-k", args.test_filter])

        pytest_args.extend(extra_args)

        print(f"\nRunning: {' '.join(pytest_args)}\n")
        result = subprocess.run(pytest_args)
        return result.returncode

    finally:
        if server_process:
            print("\nStopping server...")
            server_process.terminate()
            try:
                server_process.wait(timeout=5.0)
            except subprocess.TimeoutExpired:
                server_process.kill()


if __name__ == "__main__":
    sys.exit(main())
