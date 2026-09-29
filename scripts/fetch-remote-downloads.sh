#!/usr/bin/env bash
# Backfill storage/files/downloads/ from the bridge's GitHub release, or the
# deployed host if that is unavailable.
#
# Why this exists: the admin Bridge Setup page and the profile connect snippet
# link same-origin `/files/downloads/<asset>` (and its `.sha256`), and the
# image bakes the whole storage/ tree it is built from. storage/files/downloads/
# is gitignored, and each platform's bridge binary can only be BUILT on that
# platform's toolchain (the Windows exe via cargo-xwin, the DMG on a Mac), so
# any single machine is missing the artifacts the others produced. Building an
# image from such a tree silently 404s every other platform's download link.
#
# For every asset those pages link, this script keeps whatever is already
# staged locally (a fresh local build wins) and downloads the rest, verifying
# each file against the release's SHA256SUMS and writing the `.sha256` sidecar
# the pages link.
#
# The GitHub release is tried FIRST, and the deployed host only as a fallback.
# Sourcing from the host makes the live site the source of truth for its own
# next version: a deploy republishes whatever that host happens to be serving,
# so a bad artifact survives every later deploy and the release that built it
# is never consulted. The release is immutable, cosign-signed, and produced by
# .github/workflows/release.yml on the platform that can build each binary.
#
# Release series: the bridge ships as `bridge-v<version>` (lockstep with the
# workspace, so the version is read from bridge/Cargo.toml); the `v<version>`
# series carries the gateway tarballs and is never a download source here.
# DOWNLOADS_RELEASE_TAG overrides the tag — pin an older `bridge-vX.Y.Z` when
# rebuilding an older image. Release assets carry one SHA256SUMS, not
# per-asset sidecars; the host fallback may carry either.
#
# Fetching from the release needs an authenticated `gh`. Without one this falls
# back to the host and says so; it does not fail, because a developer without
# gh auth must still be able to stage.
#
# A missing asset WARNS and continues: no host can build every platform, so a
# hard failure would block every Linux run on the macOS DMG.
# DOWNLOADS_STRICT=1 restores the hard failure and is the release mode: every
# asset comes from the named release and nothing else — no host fallback (the
# host serves the PREVIOUS release, so a transient gh failure would silently
# ship an old binary) and no "local build wins" (the release is the build).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DOWNLOADS_DIR="$REPO_ROOT/storage/files/downloads"
BASE="${DOWNLOADS_REMOTE_BASE:-https://internal.systemprompt.io/files/downloads}"
BASE="${BASE%/}"
REPO="${DOWNLOADS_GH_REPO:-systempromptio/systemprompt-internal}"
BRIDGE_VERSION="$(sed -n 's/^version = "\([0-9][0-9.]*\)"/\1/p' "$REPO_ROOT/bridge/Cargo.toml" | head -1)"
RELEASE_TAG="${DOWNLOADS_RELEASE_TAG:-bridge-v$BRIDGE_VERSION}"
STRICT="${DOWNLOADS_STRICT:-0}"

case "$RELEASE_TAG" in
    bridge-v[0-9]*) ;;
    *) echo "ERROR: DOWNLOADS_RELEASE_TAG=$RELEASE_TAG is not a bridge-v<version> release." >&2; exit 1 ;;
esac

GH_SOURCE=""
if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1 &&
    gh release view "$RELEASE_TAG" --repo "$REPO" >/dev/null 2>&1; then
    GH_SOURCE="$RELEASE_TAG"
fi
if [ "$STRICT" = "1" ]; then
    [ -n "$GH_SOURCE" ] || { echo "ERROR: DOWNLOADS_STRICT=1 needs an authenticated gh and a published $RELEASE_TAG release on $REPO." >&2; exit 1; }
    echo "downloads: STRICT — every asset from the $GH_SOURCE release on $REPO, verified against its SHA256SUMS; no host fallback"
elif [ -n "$GH_SOURCE" ]; then
    echo "downloads: sourcing from the $GH_SOURCE release on $REPO (host $BASE is the fallback)"
else
    echo "downloads: no authenticated gh, or no $RELEASE_TAG release on $REPO; sourcing from $BASE" >&2
fi

# Every asset a page links. Checksummed ones get a `.sha256` sidecar written
# beside them; install.sh has none.
CHECKSUMMED=(
    systemprompt-internal-bridge-windows.exe
    systemprompt-internal-bridge-linux-x86_64.tar.gz
    systemprompt-internal-bridge-macos.dmg
)
PLAIN=(
    install.sh
)
# Published but linked by no page: preserved when present, skipped without
# error when absent, and never a reason for DOWNLOADS_STRICT=1 to stop — a
# strict run must not be blocked on an artifact nothing links. The macOS zip is
# the self-updater's artifact; the updater reads it from the release itself.
OPTIONAL_CHECKSUMMED=(
    systemprompt-internal-bridge-linux-aarch64.tar.gz
    systemprompt-internal-bridge-macos.zip
)

if command -v sha256sum >/dev/null 2>&1; then
    SHA_CHECK=(sha256sum -c)
else
    SHA_CHECK=(shasum -a 256 -c)
fi

mkdir -p "$DOWNLOADS_DIR"

SUMS=""
if [ -n "$GH_SOURCE" ]; then
    SUMS="$(mktemp)"
    trap 'rm -f "$SUMS"' EXIT
    gh release download "$GH_SOURCE" --repo "$REPO" --pattern SHA256SUMS --output "$SUMS" --clobber \
        || { echo "ERROR: release $GH_SOURCE has no SHA256SUMS." >&2; exit 1; }
fi

if [ "$STRICT" = "1" ]; then
    for asset in "${CHECKSUMMED[@]}" "${OPTIONAL_CHECKSUMMED[@]}" "${PLAIN[@]}"; do
        rm -f "$DOWNLOADS_DIR/$asset" "$DOWNLOADS_DIR/$asset.sha256"
    done
fi
missing=()
FETCHED_FROM=""

fetch() { # fetch <name> -> 0 fetched, 1 not found at either source
    local name="$1" tmp
    tmp="$(mktemp -d)"
    FETCHED_FROM=""
    # Why the retry: GitHub's asset download 5xxs often enough that one file of
    # a release can be lost to a blip; strict mode then fails instead of
    # falling through to the host.
    if [ -n "$GH_SOURCE" ] &&
        { gh release download "$GH_SOURCE" --repo "$REPO" --pattern "$name" \
              --dir "$tmp" --clobber >/dev/null 2>&1 ||
          { sleep 3 && gh release download "$GH_SOURCE" --repo "$REPO" --pattern "$name" \
              --dir "$tmp" --clobber >/dev/null 2>&1; }; } &&
        [ -f "$tmp/$name" ]; then
        mv "$tmp/$name" "$DOWNLOADS_DIR/$name"
        chmod 0644 "$DOWNLOADS_DIR/$name"
        rm -rf "$tmp"
        FETCHED_FROM="the $GH_SOURCE release"
        return 0
    fi
    if [ "$STRICT" != "1" ] && curl -fsSL -o "$tmp/$name" "$BASE/$name" 2>/dev/null; then
        mv "$tmp/$name" "$DOWNLOADS_DIR/$name"
        chmod 0644 "$DOWNLOADS_DIR/$name"
        rm -rf "$tmp"
        FETCHED_FROM="$BASE"
        return 0
    fi
    rm -rf "$tmp"
    return 1
}

# Write <asset>.sha256 from the release's SHA256SUMS, or from the host's own
# sidecar when the asset came from the host. 1 when neither names the asset.
sidecar() {
    local asset="$1"
    if [ "$FETCHED_FROM" != "$BASE" ] && [ -n "$SUMS" ] && grep -q " \*\{0,1\}$asset\$" "$SUMS"; then
        grep " \*\{0,1\}$asset\$" "$SUMS" > "$DOWNLOADS_DIR/$asset.sha256"
        return 0
    fi
    [ "$FETCHED_FROM" = "$BASE" ] && fetch "$asset.sha256" && FETCHED_FROM="$BASE"
}

stage_checksummed() { # stage_checksummed <asset> -> 0 staged, 1 not published
    local asset="$1"
    fetch "$asset" || return 1
    local from="$FETCHED_FROM"
    sidecar "$asset" || {
        echo "ERROR: no checksum for fetched $asset from $from — refusing to stage it." >&2
        rm -f "$DOWNLOADS_DIR/$asset" "$DOWNLOADS_DIR/$asset.sha256"
        exit 1
    }
    (cd "$DOWNLOADS_DIR" && "${SHA_CHECK[@]}" "$asset.sha256" >/dev/null) || {
        echo "ERROR: checksum mismatch for fetched $asset — refusing to stage it." >&2
        rm -f "$DOWNLOADS_DIR/$asset" "$DOWNLOADS_DIR/$asset.sha256"
        exit 1
    }
    echo "==> $asset: fetched from $from and verified"
}

keep_local() {
    local asset="$1"
    if [ ! -f "$DOWNLOADS_DIR/$asset.sha256" ]; then
        (cd "$DOWNLOADS_DIR" && { sha256sum "$asset" 2>/dev/null \
            || shasum -a 256 "$asset"; } > "$asset.sha256")
    fi
    echo "==> $asset: already staged locally, keeping it"
}

for asset in "${CHECKSUMMED[@]}"; do
    if [ -f "$DOWNLOADS_DIR/$asset" ]; then
        keep_local "$asset"
    elif ! stage_checksummed "$asset"; then
        rm -f "$DOWNLOADS_DIR/$asset" "$DOWNLOADS_DIR/$asset.sha256"
        missing+=("$asset")
    fi
done

for asset in "${OPTIONAL_CHECKSUMMED[@]}"; do
    if [ -f "$DOWNLOADS_DIR/$asset" ]; then
        keep_local "$asset"
    elif ! stage_checksummed "$asset"; then
        rm -f "$DOWNLOADS_DIR/$asset" "$DOWNLOADS_DIR/$asset.sha256"
        echo "==> $asset: not published anywhere yet, skipping (optional)"
    fi
done

for asset in "${PLAIN[@]}"; do
    if [ -f "$DOWNLOADS_DIR/$asset" ]; then
        echo "==> $asset: already staged locally, keeping it"
    elif fetch "$asset"; then
        echo "==> $asset: fetched from $FETCHED_FROM"
    else
        missing+=("$asset")
    fi
done

if [ "${#missing[@]}" -gt 0 ]; then
    echo >&2
    echo "WARNING: missing (not staged locally, not in ${GH_SOURCE:-any release}, not on $BASE):" >&2
    for m in "${missing[@]}"; do echo "    $m" >&2; done
    echo "Download links for these will 404 on the deployed site. The" >&2
    echo "$RELEASE_TAG release is cut by .github/workflows/release.yml; for a local" >&2
    echo "build use just bridge-package-linux / bridge-package-windows (dist/)." >&2
    if [ "$STRICT" = "1" ]; then
        echo "DOWNLOADS_STRICT=1 set — refusing to continue." >&2
        exit 1
    fi
    echo "Continuing anyway (set DOWNLOADS_STRICT=1 to make this fatal)." >&2
fi

if [ "$STRICT" = "1" ]; then
    staged=()
    for asset in "${CHECKSUMMED[@]}" "${OPTIONAL_CHECKSUMMED[@]}" "${PLAIN[@]}"; do
        [ -f "$DOWNLOADS_DIR/$asset" ] && staged+=("$asset")
    done
    for asset in "${OPTIONAL_CHECKSUMMED[@]}"; do
        [ -f "$DOWNLOADS_DIR/$asset" ] && continue
        grep -q " \*\{0,1\}$asset\$" "$SUMS" || continue
        echo "ERROR: $asset is published in $GH_SOURCE (listed in SHA256SUMS) but was not fetched — the image would serve a stale copy." >&2
        exit 1
    done
    pattern="$(printf ' \\*?%s$\n' "${staged[@]}" | paste -sd'|' -)"
    expected="$(grep -E "$pattern" "$SUMS")"
    [ "$(printf '%s\n' "$expected" | grep -c .)" -eq "${#staged[@]}" ] \
        || { echo "ERROR: SHA256SUMS of $GH_SOURCE does not list every staged asset:" >&2; printf '    %s\n' "${staged[@]}" >&2; exit 1; }
    (cd "$DOWNLOADS_DIR" && printf '%s\n' "$expected" | "${SHA_CHECK[@]}" -) \
        || { echo "ERROR: a staged asset does not match the $GH_SOURCE SHA256SUMS — refusing to ship it." >&2; exit 1; }
    echo "==> strict: ${#staged[@]} assets match the $GH_SOURCE SHA256SUMS"
fi

echo "==> downloads complete: $(ls "$DOWNLOADS_DIR" | tr '\n' ' ')"
