#!/usr/bin/env bash
# The two Windows bridge failure modes that only show up on a user's machine —
# refuse to ship either. Shared by scripts/package-bridge-windows.sh (cargo-xwin
# cross build) and the release workflow's native windows-latest build.
#
#   1. A dynamic import of WebView2Loader.dll: webview2-com-sys only links the
#      loader statically on msvc targets. We ship a bare .exe, so a -gnu build
#      dies at process start before main() runs.
#   2. No .rsrc section: winresource silently degraded to a warning and the
#      branded icon/version info was dropped.
#
# Tools: grep -a for the import string; llvm-objdump for the section table,
# because binutils objdump is not on a Windows runner while llvm-tools (pinned
# in rust-toolchain.toml) ships llvm-objdump on every platform.
#
# Usage: check-bridge-windows-exe.sh <path-to-exe>
set -euo pipefail

BIN="${1:?usage: check-bridge-windows-exe.sh <exe>}"
[ -f "$BIN" ] || { echo "ERROR: $BIN does not exist" >&2; exit 1; }

if grep -aq "WebView2Loader.dll" "$BIN"; then
    echo "ERROR: $BIN dynamically imports WebView2Loader.dll — the loader was" >&2
    echo "not statically linked (wrong target?). Refusing to stage it." >&2
    exit 1
fi

if command -v objdump >/dev/null 2>&1; then
    OBJDUMP=objdump
else
    sysroot="$(rustc --print sysroot)"
    OBJDUMP="$(find "$sysroot/lib/rustlib" -name 'llvm-objdump*' -type f 2>/dev/null | head -1)"
    [ -n "$OBJDUMP" ] || {
        echo "ERROR: neither objdump nor llvm-objdump found (rustup component add llvm-tools)" >&2; exit 1; }
fi
if ! "$OBJDUMP" -h "$BIN" | grep -qi '\.rsrc'; then
    echo "ERROR: $BIN has no .rsrc section — the branded icon/version resource" >&2
    echo "was dropped (rc failure?). Refusing to stage it." >&2
    exit 1
fi
echo "check-bridge-windows-exe: $BIN ok (static WebView2 loader, .rsrc present)"
