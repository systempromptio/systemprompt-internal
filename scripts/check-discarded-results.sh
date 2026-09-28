#!/usr/bin/env bash
# No fallible value may be silently thrown away in extensions/ or bridge/src:
# `let _ =`, a trailing `.ok();`, `drop(<call>)`, or `unwrap_or_default()` on a
# Result. A swallowed error is an audit gap nobody sees. The scanner lives in
# core (scripts/rust-contracts) so both repos enforce one definition; a
# `// Why: discard-ok: <reason>` line above the statement carves it out.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
# Why: the sibling is optional on a developer machine (the server builds from
# crates.io while [patch.crates-io] is dormant), so its absence skips loudly
# there. CI always links it (.github/actions/core-checkout), so under CI a
# missing sibling is a broken job, not a skip.
if [ ! -f "$root/../systemprompt-core/scripts/check-discarded-results.sh" ]; then
    if [ "${CI:-}" = "true" ]; then
        echo "check-discarded-results.sh: ../systemprompt-core/scripts/check-discarded-results.sh not found under CI; .github/actions/core-checkout must run first" >&2
        exit 1
    fi
    echo "check-discarded-results.sh: SKIPPED — no ../systemprompt-core checkout with scripts/check-discarded-results.sh (just core-checkout provides one)"
    exit 0
fi
exec bash "$root/../systemprompt-core/scripts/check-discarded-results.sh" "$root/extensions" "$root/bridge/src"
