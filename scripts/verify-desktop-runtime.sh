#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TAURI_DIR="$ROOT_DIR/apps/desktop/src-tauri"
AGENT_SCRIPT="$ROOT_DIR/agent/runtime_agent.py"

echo "[1/3] cargo check"
(cd "$TAURI_DIR" && cargo check)

echo "[2/3] cargo test"
(cd "$TAURI_DIR" && cargo test)

echo "[3/3] python health check"
python3 "$AGENT_SCRIPT" --health-check

echo "All checks passed."
