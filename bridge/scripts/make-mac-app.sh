#!/usr/bin/env bash
# Wrap the astound-bridge binary in a macOS .app bundle.
#
# Usage: bridge/scripts/make-mac-app.sh [--target <triple>]
#
# Reads version from bridge/Cargo.toml, picks the matching release binary,
# generates an .icns from bridge/assets/window-icon-1024.png, and emits
# bridge/target/<triple>/release/AstoundBridge.app (or target/release/... when
# no target is given). Must run on macOS (uses sips + iconutil).
set -euo pipefail

cd "$(dirname "$0")/.."  # crate root (bridge/)

TARGET=""
if [[ "${1:-}" == "--target" ]]; then
    TARGET="${2:?--target requires a value}"
fi

ASSETS="assets"
PLIST_TEMPLATE="macos/Info.plist"

if [[ -n "$TARGET" ]]; then
    BIN="target/$TARGET/release/astound-bridge"
    OUT_DIR="target/$TARGET/release"
else
    BIN="target/release/astound-bridge"
    OUT_DIR="target/release"
fi

if [[ ! -f "$BIN" ]]; then
    echo "binary not found at $BIN — build it first: cargo build --release${TARGET:+ --target $TARGET}" >&2
    exit 1
fi

VERSION="$(awk -F'"' '/^version/ { print $2; exit }' "Cargo.toml")"
APP="$OUT_DIR/AstoundBridge.app"
CONTENTS="$APP/Contents"
MACOS_DIR="$CONTENTS/MacOS"
RES_DIR="$CONTENTS/Resources"

rm -rf "$APP"
mkdir -p "$MACOS_DIR" "$RES_DIR"

cp "$BIN" "$MACOS_DIR/astound-bridge"
chmod +x "$MACOS_DIR/astound-bridge"

sed "s/__VERSION__/$VERSION/g" "$PLIST_TEMPLATE" > "$CONTENTS/Info.plist"

# Render the .iconset from the 1024 source and pack it into AppIcon.icns.
# sips + iconutil ship with macOS.
ICON_SRC="$ASSETS/window-icon-1024.png"
ICONSET="$(mktemp -d)/AppIcon.iconset"
mkdir -p "$ICONSET"
for size in 16 32 64 128 256 512; do
    sips -z "$size" "$size" "$ICON_SRC" --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
    sips -z $((size * 2)) $((size * 2)) "$ICON_SRC" --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns -o "$RES_DIR/AppIcon.icns" "$ICONSET"
rm -rf "$(dirname "$ICONSET")"

echo "built: $APP (v$VERSION)"
echo "run with: open '$APP'  or  '$MACOS_DIR/astound-bridge' gui"
