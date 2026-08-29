#!/usr/bin/env bash
# Regenerate every SQLx offline cache in this repository deterministically.
#
#   scripts/sqlx-prepare.sh [path/to/systemprompt-cli]
#
# `cargo sqlx prepare` only emits query data for crates it actually re-expands
# in that run, and it prunes from the cache whatever it did not emit. Left to
# itself that makes the result depend on target/ state: a crate cargo considers
# fresh contributes nothing and loses every query it owns. This script removes
# that dependence:
#
#   1. every sqlx-dependent package is cleaned before the workspace prepare,
#      and again before its own per-crate prepare;
#   2. the per-crate set is derived from `cargo metadata`, not a hand list,
#      so a new extension cannot be silently skipped;
#   3. every cache is snapshotted first and restored if anything fails, so a
#      half-run never leaves a pruned cache on disk;
#   4. a cache that ends smaller than it started is reported and rejected
#      unless PREPARE_ALLOW_PRUNE=1 — removing a query is a deliberate act.
set -euo pipefail
shopt -s nullglob

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
CLI="${1:-}"

SECRETS_FILE="$ROOT/.systemprompt/profiles/local/secrets.json"
if [ ! -f "$SECRETS_FILE" ]; then
    echo "error: no local profile secrets at $SECRETS_FILE — run 'just db-up' first" >&2
    exit 1
fi
DB_URL=$(jq -r '.database_url // empty' "$SECRETS_FILE")
if [ -z "$DB_URL" ]; then
    echo "error: no database_url in $SECRETS_FILE" >&2
    exit 1
fi
PG_ISREADY=$(command -v pg_isready || true)
for candidate in /opt/homebrew/opt/libpq/bin/pg_isready /usr/local/opt/libpq/bin/pg_isready; do
    [ -z "$PG_ISREADY" ] && [ -x "$candidate" ] && PG_ISREADY="$candidate"
done
if [ -z "$PG_ISREADY" ] || ! "$PG_ISREADY" -d "$DB_URL" -t 2 >/dev/null 2>&1; then
    echo "error: database not reachable at $DB_URL — run 'just db-up' first" >&2
    exit 1
fi

# The macros expand against the live schema, so pending migrations must land
# first or a new column reads as "relation does not exist".
if [ -n "$CLI" ] && [ -x "$CLI" ]; then
    echo "Applying pending migrations..."
    "$CLI" infra db migrate --profile local
else
    echo "warning: no systemprompt binary; skipping migrate. Build first if prepare fails on a missing relation." >&2
fi

export DATABASE_URL="$DB_URL"
export SQLX_OFFLINE=false

# name<TAB>relative-dir for every workspace package that depends on sqlx.
mapfile -t SQLX_CRATES < <(
    cargo metadata --no-deps --format-version 1 \
        | jq -r --arg root "$ROOT/" '.packages[]
            | select(.dependencies[]?.name == "sqlx")
            | "\(.name)\t\(.manifest_path | sub($root; "") | sub("/Cargo.toml$"; ""))"' \
        | sort
)
# Only crates that invoke a query macro own cache entries; a crate that merely
# derives `sqlx::Type` would otherwise gain an empty or dependency-only cache.
MACRO_RE='(sqlx::)?query(_as|_scalar|_file|_file_as|_file_scalar|_with)?!'
filtered=()
for entry in "${SQLX_CRATES[@]}"; do
    # Counted rather than `grep -q`: under `pipefail` an early-exiting reader
    # SIGPIPEs the producer and the test fails at random.
    hits=$(grep -rhE "$MACRO_RE" "${entry#*	}/src" --include='*.rs' 2>/dev/null | grep -vcE '^[[:space:]]*//' || true)
    if [ "${hits:-0}" -gt 0 ]; then
        filtered+=("$entry")
    fi
done
SQLX_CRATES=("${filtered[@]}")
if [ ${#SQLX_CRATES[@]} -eq 0 ]; then
    echo "error: no crate invokes a sqlx query macro; nothing to prepare" >&2
    exit 1
fi

cache_dirs() {
    echo .sqlx
    for entry in "${SQLX_CRATES[@]}"; do
        echo "${entry#*	}/.sqlx"
    done
}

SNAP=$(mktemp -d)
for d in $(cache_dirs); do
    if [ -d "$d" ]; then
        mkdir -p "$SNAP/$d"
        cp "$d"/*.json "$SNAP/$d/" 2>/dev/null || true
    fi
done
restore() {
    for d in $(cache_dirs); do
        if [ -d "$SNAP/$d" ]; then
            rm -rf "$ROOT/$d"
            mkdir -p "$ROOT/$d"
            cp "$SNAP/$d"/*.json "$ROOT/$d/" 2>/dev/null || true
        fi
    done
}
cleanup() {
    status=$?
    if [ $status -ne 0 ]; then
        echo "prepare failed (exit $status); restoring every .sqlx cache to its previous state" >&2
        restore
    fi
    rm -rf "$SNAP"
    exit $status
}
trap cleanup EXIT

for entry in "${SQLX_CRATES[@]}"; do
    cargo clean -p "${entry%%	*}" 2>/dev/null || true
done

echo "Preparing workspace cache..."
cargo sqlx prepare --workspace -- --features governance-ssr

# Extension crates are compiled standalone by their consumers, so each carries
# its own cache in addition to contributing to the root one.
mkdir -p .sqlx
for entry in "${SQLX_CRATES[@]}"; do
    name="${entry%%	*}"
    dir="${entry#*	}"
    case "$dir" in extensions/*) ;; *) continue ;; esac
    echo "Preparing $dir..."
    cargo clean -p "$name" 2>/dev/null || true
    (cd "$dir" && cargo sqlx prepare)
    for f in "$dir"/.sqlx/*.json; do
        cp "$f" .sqlx/
    done
done

# Any cache that shrank lost queries. With every crate cleaned above that can
# only mean SQL was removed from the source; make the person say so.
pruned=0
for d in $(cache_dirs); do
    [ -d "$SNAP/$d" ] || continue
    for f in "$SNAP/$d"/*.json; do
        if [ ! -f "$d/$(basename "$f")" ]; then
            if [ $pruned -eq 0 ]; then
                echo "queries removed from the cache:" >&2
            fi
            pruned=$((pruned + 1))
            printf '  %s: %s\n' "$d" "$(jq -r '.query' "$f" | tr -s ' \n' ' ' | cut -c1-100)" >&2
        fi
    done
done
if [ $pruned -gt 0 ] && [ "${PREPARE_ALLOW_PRUNE:-0}" != "1" ]; then
    echo "error: $pruned cached quer(y/ies) disappeared. If the SQL was really deleted, re-run with PREPARE_ALLOW_PRUNE=1." >&2
    exit 1
fi

scripts/check-sqlx-cache.sh
echo "SQLx cache prepared successfully ($(ls .sqlx | wc -l) queries in the root cache)"
