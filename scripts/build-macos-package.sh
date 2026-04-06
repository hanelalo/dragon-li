#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="$ROOT_DIR/apps/desktop"

TARGET="${1:-aarch64}"
case "$TARGET" in
  aarch64)
    BUILD_SCRIPT="tauri:build:macos"
    ;;
  x86_64|intel)
    BUILD_SCRIPT="tauri:build:macos:intel"
    ;;
  universal)
    BUILD_SCRIPT="tauri:build:macos:universal"
    ;;
  *)
    echo "Unsupported target: $TARGET"
    echo "Usage: $0 [aarch64|x86_64|universal]"
    exit 1
    ;;
esac

echo "[1/3] install dependencies"
(cd "$APP_DIR" && npm install)

echo "[2/3] build frontend"
(cd "$APP_DIR" && npm run build)

echo "[3/3] build tauri macOS bundles ($TARGET)"
(cd "$APP_DIR" && npm run "$BUILD_SCRIPT")

echo "Build finished. Artifacts under:"
echo "  $APP_DIR/src-tauri/target/*/release/bundle/"
