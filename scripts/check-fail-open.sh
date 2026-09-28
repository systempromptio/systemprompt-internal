#!/usr/bin/env bash
# Guard fns must fail closed: `unwrap_or(true)`, `map_or(true, ..)`,
# `is_none_or` and a `_ => true` arm turn "could not decide" into "allow", and a
# guard that walks an inventory param must return Option/Result so an empty
# catalog withholds rather than permits. The scanner lives in core
# (scripts/rust-contracts) so both repos share one definition.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
# Why: the sibling is optional on a developer machine (the server builds from
# crates.io while [patch.crates-io] is dormant), so its absence skips loudly
# there. CI always links it (.github/actions/core-checkout), so under CI a
# missing sibling is a broken job, not a skip.
if [ ! -f "$root/../systemprompt-core/scripts/check-fail-open.sh" ]; then
    if [ "${CI:-}" = "true" ]; then
        echo "check-fail-open.sh: ../systemprompt-core/scripts/check-fail-open.sh not found under CI; .github/actions/core-checkout must run first" >&2
        exit 1
    fi
    echo "check-fail-open.sh: SKIPPED — no ../systemprompt-core checkout with scripts/check-fail-open.sh (just core-checkout provides one)"
    exit 0
fi
exec bash "$root/../systemprompt-core/scripts/check-fail-open.sh" "$root/extensions" "$root/bridge/src"
