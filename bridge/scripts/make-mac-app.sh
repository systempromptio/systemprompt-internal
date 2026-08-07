#!/usr/bin/env bash
# Wrap the systemprompt-internal-bridge binary in a macOS .app bundle.
#
# Usage: bridge/scripts/make-mac-app.sh [--target <triple> | --universal]
#
# Reads version from bridge/Cargo.toml, picks the matching release binary,
# generates an .icns from bridge/assets/window-icon-1024.png, and emits
# bridge/target/<triple>/release/Systemprompt Internal Bridge.app (or target/release/... when
# no target is given). Must run on macOS (uses sips + iconutil).
#
# --universal lipos the aarch64 and x86_64 release binaries into a single
# universal2 executable and emits to target/release/. This is what the release
# workflow ships: one .dmg that runs on both Apple Silicon and Intel Macs.
set -euo pipefail

cd "$(dirname "$0")/.."  # crate root (bridge/)

UNIVERSAL_TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)

TARGET=""
UNIVERSAL=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --target)
            TARGET="${2:?--target requires a value}"
            shift 2
            ;;
        --universal)
            UNIVERSAL=1
            shift
            ;;
        *)
            echo "unknown argument: $1 (usage: $0 [--target <triple> | --universal])" >&2
            exit 1
            ;;
    esac
done

if [[ $UNIVERSAL -eq 1 && -n "$TARGET" ]]; then
    echo "--universal and --target are mutually exclusive" >&2
    exit 1
fi

ASSETS="assets"
PLIST_TEMPLATE="macos/Info.plist"

# SLICES holds every input binary; a single entry is copied, several are lipo'd.
if [[ $UNIVERSAL -eq 1 ]]; then
    SLICES=()
    for t in "${UNIVERSAL_TARGETS[@]}"; do
        SLICES+=("target/$t/release/systemprompt-internal-bridge")
    done
    OUT_DIR="target/release"
elif [[ -n "$TARGET" ]]; then
    SLICES=("target/$TARGET/release/systemprompt-internal-bridge")
    OUT_DIR="target/$TARGET/release"
else
    SLICES=("target/release/systemprompt-internal-bridge")
    OUT_DIR="target/release"
fi

for slice in "${SLICES[@]}"; do
    if [[ ! -f "$slice" ]]; then
        if [[ $UNIVERSAL -eq 1 ]]; then
            echo "binary not found at $slice — build every slice first:" >&2
            for t in "${UNIVERSAL_TARGETS[@]}"; do
                echo "  cargo build --release --target $t" >&2
            done
        else
            echo "binary not found at $slice — build it first: cargo build --release${TARGET:+ --target $TARGET}" >&2
        fi
        exit 1
    fi
done

VERSION="$(awk -F'"' '/^version/ { print $2; exit }' "Cargo.toml")"
APP="$OUT_DIR/Systemprompt Internal Bridge.app"
CONTENTS="$APP/Contents"
MACOS_DIR="$CONTENTS/MacOS"
RES_DIR="$CONTENTS/Resources"

rm -rf "$APP"
mkdir -p "$MACOS_DIR" "$RES_DIR"

if [[ ${#SLICES[@]} -gt 1 ]]; then
    lipo -create -output "$MACOS_DIR/systemprompt-internal-bridge" "${SLICES[@]}"
else
    cp "${SLICES[0]}" "$MACOS_DIR/systemprompt-internal-bridge"
fi
chmod +x "$MACOS_DIR/systemprompt-internal-bridge"

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

echo "built: $APP (v$VERSION, $(lipo -archs "$MACOS_DIR/systemprompt-internal-bridge"))"
echo "run with: open '$APP'  or  '$MACOS_DIR/systemprompt-internal-bridge' gui"
