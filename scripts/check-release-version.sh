#!/usr/bin/env bash
# Gate: the bridge carries the same version as the workspace, and a release
# state of the tree (no active [patch.crates-io]) is fully pinned to one core
# version — bridge/Cargo.toml, bridge/CORE_REF, every core pin.
#
# On `next` the patch is active, and the pins must match the version the patch
# paths actually carry: cargo silently ignores a `[patch.crates-io]` entry whose
# version does not satisfy the requirement and resolves the published crate
# instead, so a workspace trailing core builds green having proved nothing about
# the code it ships. On `main` (patch commented out) the same check runs against
# the workspace version, which is what the release workflow re-asserts before it
# publishes anything.
set -euo pipefail
cd "$(dirname "$0")/.."

workspace=$(sed -n 's/^version = "\([0-9.]*\)"/\1/p' Cargo.toml | head -1)
bridge=$(sed -n 's/^version = "\([0-9.]*\)"/\1/p' bridge/Cargo.toml | head -1)

if [ "$workspace" != "$bridge" ]; then
    echo "FAIL: bridge/Cargo.toml is $bridge but the workspace is $workspace — run scripts/sync-release-version.sh $workspace"
    exit 1
fi

if grep -q '^\[patch\.crates-io\]' Cargo.toml; then
    # The patch path points at <core>/systemprompt; its repo root is one up.
    patch_path=$(sed -n 's|^systemprompt = { path = "\([^"]*\)".*|\1|p' Cargo.toml | head -1)
    if [ -z "$patch_path" ]; then
        echo "FAIL: [patch.crates-io] is active but carries no systemprompt path to check against"
        exit 1
    fi
    core_root=$(dirname "$patch_path")
    if [ ! -f "$core_root/Cargo.toml" ]; then
        echo "FAIL: [patch.crates-io] points at $core_root, which has no Cargo.toml."
        echo "      The patch cannot apply, so every core crate resolves from crates.io"
        echo "      and the build proves nothing. Check out core beside this repo."
        exit 1
    fi
    core=$(sed -n 's/^version = "\([0-9.]*\)"/\1/p' "$core_root/Cargo.toml" | head -1)
    if [ -z "$core" ]; then
        echo "FAIL: could not read a workspace version from $core_root/Cargo.toml"
        exit 1
    fi
    if [ "$core" != "$workspace" ]; then
        echo "FAIL: core at $core_root is $core but this workspace pins $workspace."
        echo "      Cargo ignores a patch whose version does not satisfy the pin, so all"
        echo "      core crates would resolve from crates.io at $workspace instead of the"
        echo "      checkout — run: scripts/sync-release-version.sh $core"
        exit 1
    fi
    scripts/sync-release-version.sh "$core" --check
    echo "check-release-version: pins, bridge and core checkout all agree on $core"
    exit 0
fi

scripts/sync-release-version.sh "$workspace" --check
