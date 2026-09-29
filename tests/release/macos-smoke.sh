#!/usr/bin/env bash
# Exercise a packaged bridge without enrolling or modifying the developer's
# clients. Run on Intel and Apple Silicon; no local compilation is permitted.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DMG="${1:?usage: macos-smoke.sh downloaded.dmg expected-sha256}"
EXPECTED="${2:?expected SHA-256 is required}"
[[ "$EXPECTED" =~ ^[0-9a-f]{64}$ ]]
[ "$(shasum -a 256 "$DMG" | cut -d' ' -f1)" = "$EXPECTED" ]
bash "$ROOT/scripts/verify-bridge-macos.sh" "$DMG"
WORK="$(mktemp -d)"
MOUNT="$WORK/mounted"
cleanup() {
    if mount | grep -Fq " on $MOUNT ("; then hdiutil detach "$MOUNT" -quiet; fi
    rm -rf "$WORK"
}
trap cleanup EXIT
hdiutil attach "$DMG" -readonly -nobrowse -mountpoint "$MOUNT" -quiet
ditto "$MOUNT/Systemprompt Internal Bridge.app" "$WORK/Systemprompt Internal Bridge.app"
hdiutil detach "$MOUNT" -quiet
BIN="$WORK/Systemprompt Internal Bridge.app/Contents/MacOS/systemprompt-internal-bridge"
export SYSTEMPROMPT_BRIDGE_CONFIG="$WORK/config/systemprompt-internal-bridge.toml"
export XDG_CONFIG_HOME="$WORK/config" XDG_DATA_HOME="$WORK/data" XDG_CACHE_HOME="$WORK/cache"
unset SYSTEMPROMPT_BRIDGE_PAT
mkdir -p "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$XDG_CACHE_HOME"
"$BIN" --version
"$BIN" --help > "$WORK/help.txt"
grep -Fq 'login' "$WORK/help.txt"

# Login requires a terminal even with --no-browser. Supply one with script,
# then an empty code, verifying endpoint selection without enrolling an account.
login_link() {
    local expected_url="$1"; shift
    if printf '\n' | script -q /dev/null "$BIN" login --no-browser "$@" > "$WORK/login.txt" 2>&1; then
        echo 'FAIL: an empty login unexpectedly succeeded' >&2
        exit 1
    fi
    grep -Fq "$expected_url/bridge-auth/device-link" "$WORK/login.txt"
    grep -Fq 'nothing pasted' "$WORK/login.txt"
}
login_link https://internal.systemprompt.io
login_link http://127.0.0.1:18081 --gateway http://127.0.0.1:18081
printf 'gateway_url = "http://127.0.0.1:18082"\n' > "$SYSTEMPROMPT_BRIDGE_CONFIG"
login_link http://127.0.0.1:18082
login_link http://127.0.0.1:18083 --gateway http://127.0.0.1:18083
grep -Fq 'http://127.0.0.1:18082' "$SYSTEMPROMPT_BRIDGE_CONFIG"
echo "PASS: $(uname -m) packaged CLI, built-in default gateway, explicit and saved gateway overrides, empty-login rejection"
