#!/usr/bin/env bash
# Fail if the three lockfiles (Cargo.lock, tests/Cargo.lock, bridge/Cargo.lock)
# disagree on the version of any `systemprompt*` crate. `cargo update -w`
# re-resolves the root workspace only, so a core bump that forgets tests/ or
# bridge/ leaves one workspace compiling an older core — the 0.51.0 adoption
# surfaced exactly that as an unrelated-looking compile error. A path copy and
# a registry copy of the same crate may coexist, but only at the same version.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

lockfiles=(Cargo.lock tests/Cargo.lock bridge/Cargo.lock)

# One line per (crate, version, lockfile), deduplicated.
triples=$(
    for lock in "${lockfiles[@]}"; do
        [ -f "$lock" ] || continue
        awk -v lock="$lock" '
            /^\[\[package\]\]/ { name = ""; version = "" }
            /^name = "/    { name = $3; gsub(/"/, "", name) }
            /^version = "/ {
                version = $3; gsub(/"/, "", version)
                if (name ~ /^systemprompt/) print name, version, lock
                name = ""
            }
        ' "$lock"
    done | sort -u
)

drift=$(
    printf '%s\n' "$triples" | awk '
        {
            crate = $1; ver = $2; lock = $3
            key = crate SUBSEP ver
            if (!(key in locks)) { order[crate] = order[crate] SUBSEP ver; nvers[crate]++ }
            locks[key] = (key in locks) ? locks[key] ", " lock : lock
        }
        END {
            for (crate in nvers) {
                if (nvers[crate] < 2) continue
                n = split(substr(order[crate], 2), vers, SUBSEP)
                for (i = 1; i <= n; i++) printf "  %-40s %-10s %s\n", crate, vers[i], locks[crate SUBSEP vers[i]]
            }
        }
    ' | sort
)

if [ -n "$drift" ]; then
    echo "ERROR: systemprompt crates resolved at more than one version across lockfiles:"
    printf '%s\n' "$drift"
    echo
    echo "Re-resolve every workspace after a core bump:"
    echo "  cargo update -w && cargo update -w --manifest-path tests/Cargo.toml && cargo update -w --manifest-path bridge/Cargo.toml"
    exit 1
fi

crates=$(printf '%s\n' "$triples" | awk '{print $1}' | sort -u | wc -l)
echo "✓ $crates systemprompt crates agree on version across ${#lockfiles[@]} lockfiles"
