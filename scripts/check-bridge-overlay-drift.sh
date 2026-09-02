#!/usr/bin/env bash
# Fail when core rewrites a GUI asset that bridge/web/ shadows.
#
# bridge/.cargo/config.toml sets SYSTEMPROMPT_BRIDGE_WEB_OVERLAY=web, and core's
# bin/bridge/build.rs applies it as a WHOLE-FILE REPLACEMENT: copy_tree(overlay,
# staged) runs after core's own tree is staged, so an overlay file replaces
# core's outright. It does not merge, and it does not warn.
#
# That is how v0.43.0 shipped a sign-in screen with no stylesheet. Core split
# sp-setup into a single centred card; our bridge/web/css/setup.css still
# defined only the two-pane selectors of the layout core had replaced. It kept
# shadowing core's rewritten sheet, matched nothing, and nobody found out until
# someone looked at the screen.
#
# The overlay does two different jobs and they need different treatment:
#
#   SHADOW  — a file core also has (css/brand-overrides.css). The overlay
#             replaces it whole, so it must be re-read whenever core edits its
#             copy. Recorded in bridge/web/OVERLAY_BASE.sha256 as core's hash;
#             when core edits the file the hash moves and this gate fails. That
#             is the signal that was missing.
#   ADD     — a file core does not have (i18n/en-GB/bridge.ftl, since core ships
#             only en-US). Nothing to drift against, but it is recorded as ADDED
#             so that a file *meant* to shadow, whose path is wrong, shows up as
#             a surprising new addition instead of silently overriding nothing.
#
# Reconcile, then re-record with --update. Refreshing the manifest is meant to
# be a deliberate, review-visible act.
set -uo pipefail

cd "$(dirname "$0")/.."

CORE="${CORE_REPO:-../systemprompt-core}"
CORE_WEB="$CORE/bin/bridge/web"
OVERLAY="bridge/web"
MANIFEST="$OVERLAY/OVERLAY_BASE.sha256"

# Why: not a skip. check-fork-drift.sh skips without SIBLING_REPO so a
# single-repo CI run stays green, and that is exactly how a gate stops being
# able to fail. The sibling is always present here — the root workspace's
# [patch.crates-io] and bridge/Cargo.toml both point at it, and CI materialises
# it at bridge/CORE_REF before any gate runs. If it is absent, the build is
# broken anyway and this must say so.
if [ ! -d "$CORE_WEB" ]; then
    echo "check-bridge-overlay-drift: core GUI tree '$CORE_WEB' not found." >&2
    echo "  The bridge builds from this path (bridge/Cargo.toml) — set CORE_REPO or fix the checkout." >&2
    exit 2
fi

# Overlay files, repo-relative to bridge/web/. Only the extensions core serves
# (build.rs::is_served) shadow anything; OVERLAY_BASE.sha256 is our own.
mapfile -t overlay_files < <(
    cd "$OVERLAY" && find . -type f \
        \( -name '*.css' -o -name '*.js' -o -name '*.html' -o -name '*.ftl' \) \
        -printf '%P\n' | sort
)

if [ "${#overlay_files[@]}" -eq 0 ]; then
    echo "check-bridge-overlay-drift: no overlay files — nothing to check"
    exit 0
fi

update=0
[ "${1:-}" = "--update" ] && update=1

declare -A RECORDED
if [ -f "$MANIFEST" ]; then
    while read -r hash path; do
        case "$hash" in ''|\#*) continue ;; esac
        RECORDED["$path"]="$hash"
    done < "$MANIFEST"
fi

failed=()
declare -A CURRENT
for rel in "${overlay_files[@]}"; do
    core_file="$CORE_WEB/$rel"
    if [ ! -f "$core_file" ]; then
        # An addition: core has no such file, so there is no base to drift from.
        # Still recorded, so a mistyped shadow path surfaces as a new ADDED
        # entry rather than as an override that quietly covers nothing.
        CURRENT["$rel"]="ADDED"
        if [ "${RECORDED[$rel]:-}" != "ADDED" ]; then
            failed+=("$rel: adds a file core does not have (no bin/bridge/web/$rel). Correct if it was meant to shadow one — check the path. Otherwise re-record.")
        fi
        continue
    fi
    hash=$(sha256sum "$core_file" | cut -d' ' -f1)
    CURRENT["$rel"]="$hash"
    recorded="${RECORDED[$rel]:-}"
    if [ -z "$recorded" ]; then
        failed+=("$rel: no recorded base hash. Read core's bin/bridge/web/$rel, confirm the overlay still covers it, then re-record.")
    elif [ "$recorded" != "$hash" ]; then
        failed+=("$rel: core rewrote bin/bridge/web/$rel since this overlay was rebased onto it. The overlay REPLACES that file — re-read core's version and check the overlay still defines everything it needs, then re-record.")
    fi
done

# An entry for a file no longer in the overlay is stale bookkeeping; drop it so
# the manifest can only describe overlay files that actually exist.
for rel in "${!RECORDED[@]}"; do
    if [ -z "${CURRENT[$rel]+set}" ]; then
        failed+=("$rel: recorded in $MANIFEST but no longer in the overlay. Remove the entry.")
    fi
done

if [ "$update" -eq 1 ]; then
    {
        echo "# What each bridge/web/ overlay file does to core's GUI tree."
        echo "#   <sha256> <path>  shadows core's file, REPLACING it whole — when core"
        echo "#                    edits its copy the hash moves and the gate fails,"
        echo "#                    because the overlay must then be re-read against it."
        echo "#   ADDED    <path>  core has no such file; the overlay only adds it."
        echo "# Regenerate:"
        echo "#   bash scripts/check-bridge-overlay-drift.sh --update"
        for rel in "${overlay_files[@]}"; do
            [ -n "${CURRENT[$rel]:-}" ] && echo "${CURRENT[$rel]} $rel"
        done
    } > "$MANIFEST"
    echo "check-bridge-overlay-drift: recorded ${#CURRENT[@]} base hashes in $MANIFEST"
    exit 0
fi

if [ "${#failed[@]}" -gt 0 ]; then
    echo "check-bridge-overlay-drift: FAILED" >&2
    for line in "${failed[@]}"; do
        echo "  - $line" >&2
    done
    echo >&2
    echo "  After reconciling each overlay file with core's current version:" >&2
    echo "    bash scripts/check-bridge-overlay-drift.sh --update" >&2
    exit 1
fi

echo "check-bridge-overlay-drift: ${#overlay_files[@]} overlay file(s) match their recorded core base"
