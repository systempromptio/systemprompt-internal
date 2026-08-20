#!/usr/bin/env bash
# Package the branded bridge as a Linux release tarball.
#
# Produces dist/systemprompt-internal-bridge-linux-<arch>.tar.gz plus a .sha256, matching the
# asset name the admin Bridge Setup page links to. Keep asset names in lockstep
# with: extensions/web/admin/src/handlers/ssr/ssr_bridge_setup.rs
# (DOWNLOAD_BASE_URL), the ARTIFACTS map in
# storage/files/js/pages/admin-bridge-setup.js, and the build matrix in
# .github/workflows/bridge-release.yml.
#
# Releases are produced by that workflow (GitHub Releases is the download
# source of truth); this script is the shared packaging step and a local dev
# path. Overrides:
#   SKIP_BUILD=1        use an existing binary instead of building
#   BRIDGE_BIN=<path>   path to a prebuilt binary (implies SKIP_BUILD)
#   ASSET_ARCH=<arch>   override the arch suffix (defaults to uname -m)
#   PUBLISH=0           skip copying into storage/files/downloads/
#
# Verify the result on a machine with no config using: just clean-client
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BRIDGE_DIR="$REPO_ROOT/bridge"
DIST_DIR="$REPO_ROOT/dist"
BIN_NAME="systemprompt-internal-bridge"

if [ -z "${ASSET_ARCH:-}" ]; then
    ARCH="$(uname -m)"
    case "$ARCH" in
        x86_64)  ASSET_ARCH="x86_64" ;;
        aarch64|arm64) ASSET_ARCH="aarch64" ;;
        *) echo "ERROR: unsupported host arch '$ARCH' — build on x86_64 or aarch64." >&2; exit 1 ;;
    esac
fi
ASSET="${BIN_NAME}-linux-${ASSET_ARCH}.tar.gz"
if [ -n "${BRIDGE_BIN:-}" ]; then
    BIN="$BRIDGE_BIN"
    SKIP_BUILD=1
else
    BIN="$BRIDGE_DIR/target/release/$BIN_NAME"
fi

# ── Build ─────────────────────────────────────────────────────────────────────
# The bridge is a standalone workspace (GUI deps, own release cadence), so it is
# built directly rather than through the build coordinator, which keys on the
# main workspace's fingerprint.
if [ "${SKIP_BUILD:-0}" != "1" ]; then
    echo "==> building $BIN_NAME (release)"
    (cd "$BRIDGE_DIR" && cargo build --release)
fi
[ -f "$BIN" ] || { echo "ERROR: $BIN missing. Run without SKIP_BUILD=1/BRIDGE_BIN." >&2; exit 1; }

# ── Record runtime dependencies ───────────────────────────────────────────────
# The binary dynamically links libdbus-1 (keyring-core's secret-service store),
# libsystemd, libcap, and libgcrypt. A minimal host without them fails at exec
# with "error while loading shared libraries", before any of our error handling
# runs — so the tarball states them rather than leaving users to decode ldd.
echo "==> resolving dynamic dependencies"
SONAMES="$(ldd "$BIN" | awk '{print $1}' | grep -E '^lib' | sort -u | tr '\n' ' ')"
if ldd "$BIN" | grep -q "not found"; then
    echo "ERROR: the build host itself is missing libraries:" >&2
    ldd "$BIN" | grep "not found" >&2
    exit 1
fi

# ── Stage ─────────────────────────────────────────────────────────────────────
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
PKG="$STAGE/${BIN_NAME}-linux-${ASSET_ARCH}"
mkdir -p "$PKG"
install -m 0755 "$BIN" "$PKG/$BIN_NAME"

VERSION="$(cd "$BRIDGE_DIR" && cargo metadata --no-deps --format-version 1 \
    | jq -r '.packages[0].version')"
COMMIT="$(git -C "$REPO_ROOT" rev-parse --short HEAD 2>/dev/null || echo unknown)"

cat > "$PKG/INSTALL.md" <<EOF
# Systemprompt Internal Bridge for Linux

Version ${VERSION} (${COMMIT}, ${ASSET_ARCH})

## One-line install

Prefer the installer — it verifies the checksum, installs to the right place,
and writes the environment for you:

    curl -fsSL https://internal.systemprompt.io/files/downloads/install.sh | sh

The rest of this file is the manual equivalent.

## Runtime dependencies

This binary links the following shared libraries:

    ${SONAMES}

On a minimal host, install them first or the binary will not start at all —
it fails at exec with \`error while loading shared libraries\`, before it can
print a diagnostic:

    # Debian / Ubuntu
    sudo apt-get install -y libdbus-1-3 libcap2 libgcrypt20 libsystemd0

    # RHEL / Fedora
    sudo dnf install -y dbus-libs libcap libgcrypt systemd-libs

\`libdbus-1\` is required because credentials can be stored via the freedesktop
Secret Service. Where no Secret Service provider is running, the bridge tiers
down to the kernel keyutils keyring and then to process memory, and says which
one it chose in \`${BIN_NAME} doctor\`.

## Install

    install -Dm755 ${BIN_NAME} ~/.local/bin/${BIN_NAME}

## Set up

    ${BIN_NAME} login sp-live-...   --gateway https://your-gateway
    ${BIN_NAME} install --apply --apply-schedule
    ${BIN_NAME} sync                # pull plugins, skills, agents
    ${BIN_NAME} doctor              # confirm

\`install --apply\` writes \`~/.config/systemprompt-internal/env.sh\` (ANTHROPIC_BASE_URL and
ANTHROPIC_AUTH_TOKEN) and a managed block in \`~/.profile\` that sources it.
\`--apply-schedule\` registers two systemd user units: the periodic sync timer
and \`${BIN_NAME}-proxy.service\`, which keeps the loopback inference proxy
running. Where systemd is unavailable the units are still written and the
command warns rather than fails; run the proxy by hand with \`${BIN_NAME} proxy\`.

Open a new login shell and \`claude\` works with no manual exports.

## Headless credentials

Device certificates are the supported unattended credential. Generate one,
have an admin enrol its fingerprint, then name it in the config:

    [mtls]
    cert_keystore_ref = "~/.config/systemprompt-internal/device.pem"

\`SYSTEMPROMPT_BRIDGE_DEVICE_CERT\` still works and takes precedence.

## Uninstall

    ${BIN_NAME} uninstall           # units, env.sh, and the ~/.profile block
    rm ~/.local/bin/${BIN_NAME}
EOF

# ── Pack ──────────────────────────────────────────────────────────────────────
mkdir -p "$DIST_DIR"
# --sort=name plus fixed mtime/owner keeps the archive byte-reproducible, so a
# rebuild of the same commit yields the same checksum.
tar --sort=name --owner=0 --group=0 --numeric-owner \
    --mtime="@$(git -C "$REPO_ROOT" log -1 --format=%ct 2>/dev/null || echo 0)" \
    -czf "$DIST_DIR/$ASSET" -C "$STAGE" "$(basename "$PKG")"

(cd "$DIST_DIR" && sha256sum "$ASSET" > "$ASSET.sha256")

echo
echo "==> $DIST_DIR/$ASSET"
echo "    $(cd "$DIST_DIR" && cut -d' ' -f1 "$ASSET.sha256")"
echo "    version ${VERSION} (${COMMIT})  size $(du -h "$DIST_DIR/$ASSET" | cut -f1)"
# ── Publish to the served downloads dir ──────────────────────────────────────
# The website is the download source of truth: `just deploy` ships whatever is
# in storage/files/downloads/, served at /files/downloads. The installer is
# templated so its default --download-base points at the deployed origin
# (override with INSTALL_BASE_URL for another environment). PUBLISH=0 (CI)
# skips this step.
INSTALL_BASE_URL="${INSTALL_BASE_URL:-https://internal.systemprompt.io/files/downloads}"
if [ "${PUBLISH:-1}" = "1" ]; then
    PUBLISH_DIR="$REPO_ROOT/storage/files/downloads"
    mkdir -p "$PUBLISH_DIR"
    install -m 0644 "$DIST_DIR/$ASSET" "$PUBLISH_DIR/$ASSET"
    install -m 0644 "$DIST_DIR/$ASSET.sha256" "$PUBLISH_DIR/$ASSET.sha256"
    sed "s|@DOWNLOAD_BASE@|${INSTALL_BASE_URL%/}|" \
        "$REPO_ROOT/scripts/install-bridge.sh" > "$PUBLISH_DIR/install.sh"
    chmod 0644 "$PUBLISH_DIR/install.sh"
    echo "==> published to $PUBLISH_DIR (tarball, .sha256, install.sh @ ${INSTALL_BASE_URL%/})"
fi
