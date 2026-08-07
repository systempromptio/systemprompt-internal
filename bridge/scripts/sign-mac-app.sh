#!/usr/bin/env bash
# Codesign, notarize and staple the macOS bridge .app and its .dmg.
#
# Usage: bridge/scripts/sign-mac-app.sh <path to .app|.dmg> [more paths...]
#
# Every path is signed with the hardened runtime and a secure timestamp, then
# submitted to Apple's notary service and stapled, so the artifact launches
# without a Gatekeeper prompt — and, because the ticket is stapled rather than
# looked up online, on a machine that has never seen it and is offline.
#
# Run it twice in a release: once on the .app (before the dmg is built, so the
# dmg carries a stapled bundle) and once on the finished .dmg.
#
# Required environment:
#   SIGN_IDENTITY   e.g. "Developer ID Application: Edward Burton (7FSAPLA7RX)"
#   NOTARY_KEY      path to the App Store Connect .p8 private key
#   NOTARY_KEY_ID   key id, e.g. DH57LG8P5N
#   NOTARY_ISSUER   issuer uuid from App Store Connect → Users and Access
# Optional:
#   ENTITLEMENTS    path to an entitlements plist (default: none — see below)
#   SKIP_NOTARIZE   set to 1 to sign and verify only; useful for a fast local
#                   loop, since notarization costs a network round trip of
#                   anywhere from thirty seconds to several minutes.
#
# On entitlements: the bundle deliberately ships without any. The GUI is wry /
# WKWebView, whose JIT runs in WebKit's own XPC processes rather than ours, and
# the Keychain access in apple-native-keyring-store needs no entitlement for a
# non-sandboxed Developer ID app. Grant one only against an observed failure.
set -euo pipefail

: "${SIGN_IDENTITY:?SIGN_IDENTITY is required}"
SKIP_NOTARIZE="${SKIP_NOTARIZE:-0}"

if [[ $# -eq 0 ]]; then
    echo "usage: $0 <path to .app|.dmg> [more paths...]" >&2
    exit 1
fi

if [[ "$SKIP_NOTARIZE" != "1" ]]; then
    : "${NOTARY_KEY:?NOTARY_KEY is required (or set SKIP_NOTARIZE=1)}"
    : "${NOTARY_KEY_ID:?NOTARY_KEY_ID is required (or set SKIP_NOTARIZE=1)}"
    : "${NOTARY_ISSUER:?NOTARY_ISSUER is required (or set SKIP_NOTARIZE=1)}"
fi

ENTITLEMENTS="${ENTITLEMENTS:-}"

WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

sign_one() {
    local artifact="$1"
    local -a codesign_args=(
        --force
        --timestamp
        --options runtime
        --sign "$SIGN_IDENTITY"
    )
    [[ -n "$ENTITLEMENTS" ]] && codesign_args+=(--entitlements "$ENTITLEMENTS")

    echo "==> signing $artifact"
    # The .app holds exactly one Mach-O (Contents/MacOS/systemprompt-internal-bridge)
    # and no nested frameworks or helpers, so there is nothing to sign
    # inside-out and no need for the deprecated --deep.
    codesign "${codesign_args[@]}" "$artifact"

    codesign --verify --strict --verbose=2 "$artifact"
}

# Gatekeeper assesses an app as an executable and a dmg as a mounted volume;
# asking for the wrong type reports a false failure.
assess_one() {
    local artifact="$1"
    case "$artifact" in
        *.app) spctl --assess --type exec --verbose=4 "$artifact" ;;
        *.dmg) spctl --assess --type open --context context:primary-signature --verbose=4 "$artifact" ;;
        *)     spctl --assess --verbose=4 "$artifact" ;;
    esac
}

notarize_one() {
    local artifact="$1"
    local submission="$artifact"

    # notarytool takes a dmg directly but never a bundle; zip the .app first.
    # ditto -c -k --keepParent is the only archiver Apple supports here: zip(1)
    # mangles the symlinks and resource forks inside a bundle.
    if [[ "$artifact" == *.app ]]; then
        submission="$WORK_DIR/$(basename "$artifact").zip"
        ditto -c -k --keepParent "$artifact" "$submission"
    fi

    echo "==> notarizing $artifact (this can take several minutes)"
    xcrun notarytool submit "$submission" \
        --key "$NOTARY_KEY" \
        --key-id "$NOTARY_KEY_ID" \
        --issuer "$NOTARY_ISSUER" \
        --wait \
        --timeout 30m

    echo "==> stapling $artifact"
    xcrun stapler staple "$artifact"
    xcrun stapler validate "$artifact"
}

for artifact in "$@"; do
    if [[ ! -e "$artifact" ]]; then
        echo "artifact not found: $artifact" >&2
        exit 1
    fi

    sign_one "$artifact"

    if [[ "$SKIP_NOTARIZE" == "1" ]]; then
        echo "==> SKIP_NOTARIZE=1, not submitting $artifact"
        # An un-notarized signature cannot pass assessment, so skip the check
        # rather than fail the script on an expected rejection.
        continue
    fi

    notarize_one "$artifact"
    assess_one "$artifact"
    echo "==> $artifact is signed, notarized and stapled"
done
