#!/usr/bin/env bash
# Gate: the bridge carries the same version as the workspace, and a release
# state of the tree (no active [patch.crates-io]) is fully pinned to one core
# version — bridge/Cargo.toml, bridge/CORE_REF, every core pin.
#
# On `next` the patch is active and the workspace deliberately trails core
# next, so only the bridge/workspace equality is enforced there. On `main`
# (patch commented out) the full sync check runs, which is what the release
# workflow re-asserts before it publishes anything.
set -euo pipefail
cd "$(dirname "$0")/.."

workspace=$(sed -n 's/^version = "\([0-9.]*\)"/\1/p' Cargo.toml | head -1)
bridge=$(sed -n 's/^version = "\([0-9.]*\)"/\1/p' bridge/Cargo.toml | head -1)

if [ "$workspace" != "$bridge" ]; then
    echo "FAIL: bridge/Cargo.toml is $bridge but the workspace is $workspace — run scripts/sync-release-version.sh $workspace"
    exit 1
fi

if grep -q '^\[patch\.crates-io\]' Cargo.toml; then
    echo "check-release-version: bridge and workspace agree on $workspace ([patch.crates-io] active — full pin check deferred to main)"
    exit 0
fi

scripts/sync-release-version.sh "$workspace" --check
