#!/usr/bin/env python3
"""Minimal runtime agent for desktop-runtime-core bootstrap."""

import argparse
import json
import signal
import sys
import time

_running = True


def _handle_signal(_signum, _frame):
    global _running
    _running = False


def run_health_check() -> int:
    print("ok")
    return 0


def run_server() -> int:
    signal.signal(signal.SIGTERM, _handle_signal)
    signal.signal(signal.SIGINT, _handle_signal)

    while _running:
        # Keeps the process alive for host lifecycle management.
        time.sleep(1)

    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Dragon-Li runtime agent")
    parser.add_argument("--health-check", action="store_true")
    parser.add_argument("--serve", action="store_true")
    args = parser.parse_args()

    if args.health_check:
        return run_health_check()

    if args.serve:
        return run_server()

    print(json.dumps({"ok": False, "error": "No mode selected"}))
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
