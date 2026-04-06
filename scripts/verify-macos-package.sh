#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="$ROOT_DIR/apps/desktop"
BUNDLE_ROOT="$APP_DIR/src-tauri/target"

APP_PATH="$(find "$BUNDLE_ROOT" -type d -path "*/release/bundle/macos/*.app" | head -n 1 || true)"
DMG_PATH="$(find "$BUNDLE_ROOT" -type f -path "*/release/bundle/dmg/*.dmg" | head -n 1 || true)"

if [[ -z "$APP_PATH" ]]; then
  echo "No .app artifact found under $BUNDLE_ROOT"
  exit 1
fi

if [[ -z "$DMG_PATH" ]]; then
  echo "No .dmg artifact found under $BUNDLE_ROOT"
  exit 1
fi

echo "Found app: $APP_PATH"
echo "Found dmg: $DMG_PATH"

if [[ ! -f "$APP_PATH/Contents/Info.plist" ]]; then
  echo "Missing Info.plist in app bundle"
  exit 1
fi

EXEC_NAME="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$APP_PATH/Contents/Info.plist")"
if [[ ! -x "$APP_PATH/Contents/MacOS/$EXEC_NAME" ]]; then
  echo "Missing executable in app bundle: $APP_PATH/Contents/MacOS/$EXEC_NAME"
  exit 1
fi

echo "App bundle structure looks valid"

MOUNT_DIR="/tmp/dragon-li-dmg-check-$$"
mkdir -p "$MOUNT_DIR"
ATTACH_OK=1
if hdiutil attach "$DMG_PATH" -mountpoint "$MOUNT_DIR" -nobrowse -quiet; then
  ATTACH_OK=0
fi

if [[ $ATTACH_OK -eq 0 ]]; then
  if ! find "$MOUNT_DIR" -maxdepth 2 -name "*.app" | grep -q .; then
    echo "No .app found inside mounted dmg"
    hdiutil detach "$MOUNT_DIR" >/dev/null 2>&1 || true
    rmdir "$MOUNT_DIR" >/dev/null 2>&1 || true
    exit 1
  fi
  hdiutil detach "$MOUNT_DIR" >/dev/null 2>&1 || true
  rmdir "$MOUNT_DIR" >/dev/null 2>&1 || true
  echo "DMG mount check passed"
else
  rmdir "$MOUNT_DIR" >/dev/null 2>&1 || true
  echo "DMG mount is unavailable in current environment, fallback to checksum verification"
  hdiutil verify "$DMG_PATH" >/dev/null
  echo "DMG checksum verification passed"
fi

echo "macOS package verification succeeded"
