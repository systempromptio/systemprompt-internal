#!/usr/bin/env bash
# Sync every systemprompt-core crate pin to one published core version, and
# point bridge/CORE_REF at its tag.
#
# This repository versions in LOCKSTEP with core: the workspace, the bridge
# and the image all carry the core version they build against. So the
# release entry point is scripts/sync-release-version.sh, which sets this
# repo's own version fields and calls this script with the same number; this
# script is the core-pin half on its own (what `just core-bump` and a
# `[patch.crates-io]` cycle need).
#
#   scripts/sync-core-version.sh 0.61.0          # apply
#   scripts/sync-core-version.sh 0.61.0 --check  # verify only (CI guard)
#   scripts/sync-core-version.sh --check         # verify against the root pin
#
# Covered pins:
#   Cargo.toml            systemprompt/-security/-extension
#   tests/Cargo.toml      systemprompt/-security/-api/-models (separate workspace)
#   any other systemprompt* pin in a Cargo.toml (residual sweep)
#   bridge/CORE_REF       v<version>, when [patch.crates-io] is inactive
#
# macOS + Linux compatible (no GNU-only sed flags).
set -eu

if [ "${1:-}" = "--check" ]; then
    VERSION="$(sed -n 's/^systemprompt = { version = "\([0-9.]*\)".*/\1/p' "$(dirname "$0")/../Cargo.toml" | head -1)"
    MODE=--check
else
    VERSION="${1:?usage: sync-core-version.sh <core-version> [--check] | --check}"
    MODE="${2:-apply}"
fi
cd "$(dirname "$0")/.."

case "$VERSION" in
  *[!0-9.]*|*..*|.*|*.) echo "ERROR: '$VERSION' is not a plain semver (X.Y.Z)"; exit 1 ;;
esac
IFS=. read -r MAJ MIN PATCH <<EOV
$VERSION
EOV
: "${PATCH:?ERROR: version must have three components}"

fail=0

check_or_apply() { # $1=file $2=sed-expr $3=expect-regex $4=label
    local file="$1" sedexpr="$2" expect="$3" label="$4"
    if [ "$MODE" = "--check" ]; then
        if ! grep -Eq "$expect" "$file"; then
            echo "DRIFT: $label in $file (expected /$expect/)"
            fail=1
        fi
    else
        sed -i.bak -e "$sedexpr" "$file" && rm -f "$file.bak"
        grep -Eq "$expect" "$file" || { echo "ERROR: failed to set $label in $file"; exit 1; }
    fi
}

# Cargo.toml — core crate pins.
check_or_apply Cargo.toml \
    "s|^systemprompt = { version = \"[0-9.]*\"|systemprompt = { version = \"$VERSION\"|" \
    "^systemprompt = \\{ version = \"$VERSION\"" \
    "systemprompt core pin"
check_or_apply Cargo.toml \
    "s|^systemprompt-security = { version = \"[0-9.]*\"|systemprompt-security = { version = \"$VERSION\"|" \
    "^systemprompt-security = \\{ version = \"$VERSION\"" \
    "systemprompt-security core pin"
check_or_apply Cargo.toml \
    "s|^systemprompt-extension = { version = \"[0-9.]*\"|systemprompt-extension = { version = \"$VERSION\"|" \
    "^systemprompt-extension = \\{ version = \"$VERSION\"" \
    "systemprompt-extension core pin"

# tests/Cargo.toml — the test workspace is excluded from the root workspace and
# carries its own copies of the same pins. Nothing else rewrites them, and a
# stale pin here silently disables the test workspace's [patch.crates-io].
check_or_apply tests/Cargo.toml \
    "s|^systemprompt = { version = \"[0-9.]*\"|systemprompt = { version = \"$VERSION\"|" \
    "^systemprompt = \\{ version = \"$VERSION\"" \
    "systemprompt core pin (test workspace)"
check_or_apply tests/Cargo.toml \
    "s|^systemprompt-security = { version = \"[0-9.]*\"|systemprompt-security = { version = \"$VERSION\"|" \
    "^systemprompt-security = \\{ version = \"$VERSION\"" \
    "systemprompt-security core pin (test workspace)"
check_or_apply tests/Cargo.toml \
    "s|^systemprompt-api = { version = \"[0-9.]*\"|systemprompt-api = { version = \"$VERSION\"|" \
    "^systemprompt-api = \\{ version = \"$VERSION\"" \
    "systemprompt-api core pin (test workspace)"
# The test workspace also pins crates the facade does not re-export, in the
# bare-string form.
for crate in systemprompt-models systemprompt-content systemprompt-marketplace; do
    check_or_apply tests/Cargo.toml \
        "s|^$crate = \"[0-9.]*\"|$crate = \"$VERSION\"|" \
        "^$crate = \"$VERSION\"" \
        "$crate core pin (test workspace)"
done

# Residual sweep: any core pin in any manifest that the rules above do not
# already move. A pin added to a new crate would otherwise sit stale forever,
# because no gate distinguishes a forgotten pin from a deliberate one.
#
# Both spellings count. A bare-string pin (`systemprompt-models = "0.43.0"`) is
# as load-bearing as the table form, and sweeping only the table form let a
# stale one sit behind an active patch in a sibling repo until the patch came
# off and the lockfile quietly resolved two versions of the same core crate.
# .vendor/ is where CI checks core out (a copy of core itself), not our pins.
stale=$(grep -rnE '^systemprompt[a-z-]* = ("[0-9]|\{ version = ")' --include=Cargo.toml . \
    | grep -v '/target/' | grep -v '/\.vendor/' \
    | grep -vE "= \"$VERSION\"|version = \"$VERSION\"" || true)
if [ -n "$stale" ]; then
    echo "DRIFT: core pins not on $VERSION and not covered by this script:"
    echo "$stale"
    fail=1
    [ "$MODE" = "--check" ] || exit 1
fi

# bridge/CORE_REF names the core the bridge builds against. With patches
# active it is a core SHA (just core-pin); published-core mode needs the tag.
if ! grep -qE '^\[patch\.crates-io\]' Cargo.toml; then
    if [ "$MODE" = "--check" ]; then
        [ "$(tr -d '[:space:]' < bridge/CORE_REF)" = "v$VERSION" ] || {
            echo "DRIFT: bridge/CORE_REF is $(tr -d '[:space:]' < bridge/CORE_REF), expected v$VERSION"
            fail=1
        }
    else
        printf 'v%s\n' "$VERSION" > bridge/CORE_REF
    fi
fi

if [ "$MODE" = "--check" ]; then
    [ "$fail" -eq 0 ] && echo "core sync OK: every core pin on $VERSION" || exit 1
else
    echo "core sync applied: $VERSION"
fi
