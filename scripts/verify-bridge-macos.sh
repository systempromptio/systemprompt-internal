#!/usr/bin/env bash
# Verify the exact downloaded DMG and the app a customer copies out of it.
# No execution or quarantine removal; suitable for untrusted release inputs.
set -euo pipefail

DMG="${1:?usage: verify-bridge-macos.sh path/to/systemprompt-internal-bridge-macos.dmg}"
TEAM_ID="${APPLE_TEAM_ID:-7FSAPLA7RX}"
[ "$(uname -s)" = Darwin ] || { echo 'macOS is required' >&2; exit 1; }
[ -f "$DMG" ] || { echo "missing DMG: $DMG" >&2; exit 1; }
WORK="$(mktemp -d)"
MOUNT="$WORK/mounted"
cleanup() {
    if mount | grep -Fq " on $MOUNT ("; then hdiutil detach "$MOUNT" -quiet; fi
    # WORK is created by mktemp, never supplied by the caller.
    rm -rf "$WORK"
}
trap cleanup EXIT

verify_identity() {
    codesign --verify --strict --deep "$1"
    if ! codesign -d --verbose=4 "$1" 2>&1 | grep -Fx "TeamIdentifier=$TEAM_ID"; then
        echo "FAIL: $1 is not signed by the required Developer ID team $TEAM_ID" >&2
        return 1
    fi
    xcrun stapler validate "$1"
}

verify_identity "$DMG"
spctl --assess --type open --context context:primary-signature --verbose=4 "$DMG"
hdiutil attach "$DMG" -readonly -nobrowse -mountpoint "$MOUNT" -quiet
[ "$(readlink "$MOUNT/Applications")" = /Applications ]
APP="$WORK/Systemprompt Internal Bridge.app"
ditto "$MOUNT/Systemprompt Internal Bridge.app" "$APP"
hdiutil detach "$MOUNT" -quiet
verify_identity "$APP"
spctl --assess --type exec --verbose=4 "$APP"
BIN="$APP/Contents/MacOS/systemprompt-internal-bridge"
codesign --verify --strict "$BIN"
codesign -d --verbose=4 "$BIN" 2>&1 | grep -E 'flags=.*runtime'
lipo "$BIN" -verify_arch arm64 x86_64
echo 'PASS: universal Developer ID app and DMG, both notarized and stapled'
