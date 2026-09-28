# systemprompt-internal
set dotenv-load
# Without this, `just cli ... --full-name "Test User"` word-splits the quoted
# value into two arguments before the CLI ever parses it.
set positional-arguments

# The fork-aware gates (check-fork-drift, check-dead-repository-code) compare
# against the sibling template checkout. Without this they skip silently, which
# reads as "passed" — export a default so they actually run, and let an
# already-set value win for CI or a non-standard layout.
export SIBLING_REPO := env("SIBLING_REPO", if path_exists("../systemprompt-template") == "true" { "../systemprompt-template" } else { "" })

CLI_RELEASE := "target/release/systemprompt"

# Cloud profile every deploy targets: tenant a2f658d8bc5f, Fly app
# sp-a2f658d8bc5f, served at https://internal.systemprompt.io.
# See .systemprompt/profiles/production/.
DEPLOY_PROFILE := "production"

# Use newest binary (release vs debug, whichever is most recent)
CLI := if path_exists("target/release/systemprompt") == "true" { \
    if path_exists("target/debug/systemprompt") == "true" { \
        `[ target/release/systemprompt -nt target/debug/systemprompt ] && echo target/release/systemprompt || echo target/debug/systemprompt` \
    } else { \
        "target/release/systemprompt" \
    } \
} else if path_exists("target/debug/systemprompt") == "true" { \
    "target/debug/systemprompt" \
} else { \
    "echo 'ERROR: No CLI binary found. Run: just build' && exit 1" \
}

# Default: run CLI with any arguments
default *ARGS:
    {{CLI}} "$@"

# Run CLI with full session context (profile + auth token)
cli *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    SESSION_FILE="{{justfile_directory()}}/.systemprompt/sessions/index.json"
    if [ -f "$SESSION_FILE" ]; then
        ACTIVE_KEY=$(jq -r '.active_key // "local"' "$SESSION_FILE")
        export SYSTEMPROMPT_PROFILE=$(jq -r ".sessions[\"$ACTIVE_KEY\"].profile_path // empty" "$SESSION_FILE")
        export SYSTEMPROMPT_AUTH_TOKEN=$(jq -r ".sessions[\"$ACTIVE_KEY\"].session_token // empty" "$SESSION_FILE")
    fi
    if [ -z "${SYSTEMPROMPT_PROFILE:-}" ]; then
        export SYSTEMPROMPT_PROFILE="{{justfile_directory()}}/.systemprompt/profiles/local/profile.yaml"
    fi
    exec {{CLI}} "$@"

# Get DATABASE_URL from profile secrets (for sqlx compile-time checks)
_db-url:
    @PROFILE="${SYSTEMPROMPT_PROFILE:-}"; \
    [ -n "$PROFILE" ] || PROFILE="{{justfile_directory()}}/.systemprompt/profiles/local/profile.yaml"; \
    if [ -f "$PROFILE" ]; then \
        PROFILE_DIR="$(dirname "$PROFILE")"; \
        SECRETS_PATH="$(yq -r '.secrets.secrets_path // "./secrets.json"' "$PROFILE")"; \
        if [ "${SECRETS_PATH#/}" = "$SECRETS_PATH" ]; then \
            SECRETS_FILE="$PROFILE_DIR/$SECRETS_PATH"; \
        else \
            SECRETS_FILE="$SECRETS_PATH"; \
        fi; \
        if [ -f "$SECRETS_FILE" ]; then \
            jq -r '.database_url' "$SECRETS_FILE"; \
        else \
            echo "postgres://systemprompt:systemprompt@localhost:5432/systemprompt"; \
        fi; \
    else \
        cat .systemprompt/tenants.json 2>/dev/null | jq -r '.tenants[] | select(.tenant_type == "local") | .database_url' | head -1 || echo "postgres://systemprompt:systemprompt@localhost:5432/systemprompt"; \
    fi

# ══════════════════════════════════════════════════════════════════════════════
# BUILD & CHECK
# ══════════════════════════════════════════════════════════════════════════════

# Build (Windows) - always uses offline mode
[windows]
build *FLAGS:
    $env:SQLX_OFFLINE="true"; cargo build --workspace {{FLAGS}}

# Build (Unix) - one build in flight at a time, always of the latest source;
# cargo's own incremental cache decides how much recompiles. Refuses when the
# target volume has less than BUILD_MIN_FREE_GB (default 25) free.
[unix]
build *FLAGS:
    @scripts/build-coordinator.sh run build "{{FLAGS}}" -- {{just_executable()}} _build-uncoordinated {{FLAGS}}

# What is the build/lint/test state right now? Read this before running anything.
[unix]
build-status *RECIPE:
    @scripts/build-coordinator.sh status {{RECIPE}}

# Kept for muscle memory: `just build` always compiles the current tree, so
# this is the same recipe.
[unix]
build-force *FLAGS:
    @scripts/build-coordinator.sh run build "{{FLAGS}}" -- {{just_executable()}} _build-uncoordinated {{FLAGS}}

# The real build. Call `just build` instead - this one skips coordination.
[unix]
_build-uncoordinated *FLAGS:
    #!/usr/bin/env bash
    set -euo pipefail
    # Explicit offline validation must never migrate an existing database.
    if [ "${SQLX_OFFLINE:-}" = "true" ]; then
        export CC="${CC:-clang}"
        export CXX="${CXX:-clang++}"
        export RUSTFLAGS="${RUSTFLAGS:--D warnings}"
        cargo build --workspace --locked {{FLAGS}}
        exit 0
    fi
    # Default to the `local` profile when one is set up but no SYSTEMPROMPT_PROFILE
    # is explicitly exported — keeps the in-build migrate step from failing
    # with "Profile '' not found" on a fresh clone where setup-local writes
    # secrets.json before invoking `just build`.
    SECRETS_FILE_DEFAULT_PROFILE="{{justfile_directory()}}/.systemprompt/profiles/local/secrets.json"
    if [ -z "${SYSTEMPROMPT_PROFILE:-}" ] && [ -f "$SECRETS_FILE_DEFAULT_PROFILE" ]; then
        export SYSTEMPROMPT_PROFILE="local"
    else
        export SYSTEMPROMPT_PROFILE="${SYSTEMPROMPT_PROFILE:-}"
    fi
    # aws-lc-sys refuses to build with GCC <10 due to bug #95189.
    # Force clang if available so release (LTO) builds succeed.
    if command -v clang >/dev/null 2>&1; then
        export CC="${CC:-clang}"
        export CXX="${CXX:-clang++}"
    fi
    SECRETS_FILE="{{justfile_directory()}}/.systemprompt/profiles/local/secrets.json"
    USE_OFFLINE=false
    db_reachable() {
        local url="$1"
        local pgcmd=""
        if command -v pg_isready >/dev/null 2>&1; then pgcmd="pg_isready"
        elif [ -x /opt/homebrew/opt/libpq/bin/pg_isready ]; then pgcmd="/opt/homebrew/opt/libpq/bin/pg_isready"
        elif [ -x /usr/local/opt/libpq/bin/pg_isready ]; then pgcmd="/usr/local/opt/libpq/bin/pg_isready"
        fi
        if [ -n "$pgcmd" ]; then
            "$pgcmd" -d "$url" -t 2 >/dev/null 2>&1 && return 0 || return 1
        fi
        local hostport="${url#*@}"; hostport="${hostport%%/*}"
        local host="${hostport%:*}"; local port="${hostport##*:}"
        [ "$port" = "$host" ] && port=5432
        (exec 3<>/dev/tcp/"$host"/"$port") >/dev/null 2>&1 && { exec 3<&-; exec 3>&-; return 0; } || return 1
    }
    if [ -f "$SECRETS_FILE" ]; then
        DB_URL=$(sed -n 's/.*"database_url"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$SECRETS_FILE" 2>/dev/null | head -1)
        if [ -n "$DB_URL" ] && [ "$DB_URL" != "null" ]; then
            if db_reachable "$DB_URL"; then
                export DATABASE_URL="$DB_URL"
                echo "Using database: $DB_URL"
            else
                echo "Database not reachable, using offline mode"
                USE_OFFLINE=true
            fi
        else
            echo "No database_url in secrets, using offline mode"
            USE_OFFLINE=true
        fi
    else
        echo "No local profile secrets found, using offline mode"
        USE_OFFLINE=true
    fi
    # Sync DATABASE_URL to MCP extension directories for sqlx compile-time checks
    if [ "$USE_OFFLINE" = "false" ]; then
        for dir in extensions/mcp/*/; do
            if [ -f "$dir/Cargo.toml" ]; then
                echo "DATABASE_URL=$DATABASE_URL" > "$dir/.env"
            fi
        done
    fi
    cargo update systemprompt --quiet 2>/dev/null || true
    if [ "$USE_OFFLINE" = "true" ]; then
        SQLX_OFFLINE=true cargo build --workspace {{FLAGS}}
    else
        # Apply pending schema migrations before the online sqlx compile-time
        # check sees the live DB. Build the CLI in offline mode first so
        # drift between checked-in `.sqlx/` and the unmigrated live schema
        # can't deadlock the bootstrap.
        echo "Applying pending migrations before online build..."
        SQLX_OFFLINE=true cargo build --bin systemprompt --quiet
        target/debug/systemprompt infra db migrate
        SQLX_OFFLINE=false cargo build --workspace {{FLAGS}}
    fi

# Clippy (Windows) - always uses offline mode
[windows]
clippy *FLAGS: lint-no-synthesis lint-no-untyped-admin lint-gates
    $env:SQLX_OFFLINE="true"; cargo clippy --workspace {{FLAGS}} -- -D warnings

# Clippy (Unix) - single-flight, same coordinator as `just build`
[unix]
clippy *FLAGS:
    @scripts/build-coordinator.sh run clippy "{{FLAGS}}" -- {{just_executable()}} _clippy-uncoordinated {{FLAGS}}

# The real clippy. Call `just clippy` instead - this one skips coordination.
[unix]
_clippy-uncoordinated *FLAGS: lint-no-synthesis lint-no-untyped-admin lint-gates
    #!/usr/bin/env bash
    set -euo pipefail
    SECRETS_FILE="{{justfile_directory()}}/.systemprompt/profiles/local/secrets.json"
    USE_OFFLINE=false
    db_reachable() {
        local url="$1"
        local pgcmd=""
        if command -v pg_isready >/dev/null 2>&1; then pgcmd="pg_isready"
        elif [ -x /opt/homebrew/opt/libpq/bin/pg_isready ]; then pgcmd="/opt/homebrew/opt/libpq/bin/pg_isready"
        elif [ -x /usr/local/opt/libpq/bin/pg_isready ]; then pgcmd="/usr/local/opt/libpq/bin/pg_isready"
        fi
        if [ -n "$pgcmd" ]; then
            "$pgcmd" -d "$url" -t 2 >/dev/null 2>&1 && return 0 || return 1
        fi
        local hostport="${url#*@}"; hostport="${hostport%%/*}"
        local host="${hostport%:*}"; local port="${hostport##*:}"
        [ "$port" = "$host" ] && port=5432
        (exec 3<>/dev/tcp/"$host"/"$port") >/dev/null 2>&1 && { exec 3<&-; exec 3>&-; return 0; } || return 1
    }
    if [ -f "$SECRETS_FILE" ]; then
        DB_URL=$(sed -n 's/.*"database_url"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$SECRETS_FILE" 2>/dev/null | head -1)
        if [ -n "$DB_URL" ] && [ "$DB_URL" != "null" ]; then
            if db_reachable "$DB_URL"; then
                export DATABASE_URL="$DB_URL"
            else
                USE_OFFLINE=true
            fi
        else
            USE_OFFLINE=true
        fi
    else
        USE_OFFLINE=true
    fi
    if [ "$USE_OFFLINE" = "true" ]; then
        SQLX_OFFLINE=true cargo clippy --workspace {{FLAGS}} -- -D warnings
    else
        SQLX_OFFLINE=false cargo clippy --workspace {{FLAGS}} -- -D warnings
    fi
    # tests/ is a standalone workspace and is not covered by --workspace. Its
    # warnings are not denied yet (quality.yml never denied them either);
    # tightening it to -D warnings is its own change.
    SQLX_OFFLINE=true cargo clippy --manifest-path tests/Cargo.toml --workspace {{FLAGS}}
    # bridge/ is a standalone workspace and is not covered by --workspace
    cargo clippy --manifest-path bridge/Cargo.toml --all-targets {{FLAGS}} -- -D warnings
    # Why: the GUI is cfg'd to windows|macos, so a Linux clippy never compiles
    # it and a core API break there first surfaces in release.yml's mac/win
    # jobs (astound 0.53.0). Clippy does not link, so the Windows cfg set checks
    # on Linux with no mingw toolchain; macOS-only code needs a mac runner.
    if [ "$(uname -s)" = "Linux" ]; then
        rustup target add x86_64-pc-windows-gnu
        cargo clippy --manifest-path bridge/Cargo.toml --all-targets --target x86_64-pc-windows-gnu {{FLAGS}} -- -D warnings
    fi

# Unit tests: extensions/web/admin (main workspace) + the tests/ workspace.
# If sqlx offline errors appear, run `just prepare` first to refresh .sqlx.
test-unit:
    @scripts/build-coordinator.sh run test-unit "" -- {{just_executable()}} _test-unit-uncoordinated

# Why: without --no-fail-fast nextest stops at the first failing test, so a
# Gates round reports one finding and the next one waits for another round.
# Every tier runs to completion and lists every failure at once.
#
# Why: the tests-workspace tiers pass --workspace and pick their crates with a
# nextest filter rather than -p. A -p set unifies features for that set alone,
# so each tier recompiled systemprompt-web-admin and everything above it;
# --workspace unifies once and later tiers reuse the build.
#
# The two root-workspace runs are the in-crate test dirs this repo still has
# (extensions/web/admin/tests, extensions/web/tests); they move into tests/
# with the test-foundation stage of the astound backport.
_test-unit-uncoordinated:
    cargo nextest run --no-fail-fast --no-tests=pass -p systemprompt-web-admin --tests
    cargo nextest run --no-fail-fast --no-tests=pass -p systemprompt-web-extension --tests
    cargo nextest run --no-fail-fast --manifest-path tests/Cargo.toml --workspace -E 'package(mcp-unit-tests) | package(web-unit-tests)'

# DB-backed integration tests. Creates/drops throwaway mcp_ext_test_*
# databases on the maintenance DB; the harness guard refuses any database
# name that is not 'test', 'postgres', or '*_test'. Falls back to the local
# profile's server with the database swapped to 'postgres'.
test-integration:
    @scripts/build-coordinator.sh run test-integration "" -- {{just_executable()}} _test-integration-uncoordinated

_test-integration-uncoordinated:
    #!/usr/bin/env bash
    set -euo pipefail
    db_env="$({{just_executable()}} _test-database-url)"
    eval "$db_env"
    cargo nextest run --no-fail-fast --manifest-path tests/Cargo.toml --workspace -E 'package(mcp-integration-tests) | package(web-integration-tests) | package(admin-db-core-tests) | package(admin-db-config-tests) | package(schema-upgrade-tests)'

# HTTP contract suite: drives every admin route under three principals and
# diffs the result against tests/contract/admin/baseline.txt. Same throwaway-
# database convention as test-integration. A status change fails the run; if
# it is deliberate, re-record with UPDATE_CONTRACT_BASELINE=1 and list it in
# the PR.
test-contract *ARGS:
    @scripts/build-coordinator.sh run test-contract "$*" -- {{just_executable()}} _test-contract-uncoordinated "$@"

_test-contract-uncoordinated *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    db_env="$({{just_executable()}} _test-database-url)"
    eval "$db_env"
    # Why: the contract suite self-skips when no database is reachable, and it
    # carries the governance privilege-escalation check. This turns that skip
    # into a failure, so a missing database can never be read as a pass.
    export SYSTEMPROMPT_REQUIRE_DB=1
    cargo nextest run --no-fail-fast --manifest-path tests/Cargo.toml --workspace -E 'package(admin-contract-tests)' "$@"

# End-to-end tier of `just test` (Tier A of `just e2e`, below): the full
# router in-process with the real odoo and agent MCP binaries over the wire.
# Builds a MISSING MCP binary, never a stale one — see `just e2e` for why a
# red run right after a core bump wants those rebuilt first.
test-e2e:
    @scripts/build-coordinator.sh run test-e2e "" -- {{just_executable()}} _test-e2e-uncoordinated

_test-e2e-uncoordinated:
    #!/usr/bin/env bash
    set -euo pipefail
    unset ODOO_URL ODOO_DB
    missing=()
    for bin in systemprompt-mcp-odoo systemprompt-mcp-agent; do
        [ -x "target/release/$bin" ] || [ -x "target/debug/$bin" ] || missing+=(-p "$bin")
    done
    if [ "${#missing[@]}" -gt 0 ]; then
        echo "building the MCP binaries the e2e suite spawns: ${missing[*]}"
        cargo build "${missing[@]}"
    fi
    db_env="$({{just_executable()}} _test-database-url)"
    eval "$db_env"
    cargo nextest run --no-fail-fast --manifest-path tests/Cargo.toml --workspace -E 'package(e2e-tests)'

# Prints `export SYSTEMPROMPT_TEST_DATABASE_URL=...` for the DB-backed tiers:
# an explicit value wins, else the local profile's server with the database
# swapped to the `postgres` maintenance DB.
# Why it fails instead of falling through: with no URL the suites have a
# reason to skip, and a skipped DB tier reports the same green as one that ran
# every assertion.
_test-database-url:
    #!/usr/bin/env bash
    set -euo pipefail
    url="${SYSTEMPROMPT_TEST_DATABASE_URL:-}"
    if [ -z "$url" ] && [ -f .systemprompt/profiles/local/secrets.json ]; then
        url=$(python3 -c "
    import json, urllib.parse as up
    u = up.urlsplit(json.load(open('.systemprompt/profiles/local/secrets.json'))['database_url'])
    print(up.urlunsplit((u.scheme, u.netloc, '/postgres', '', '')))")
    fi
    if [ -z "$url" ]; then
        echo "No test database. Set SYSTEMPROMPT_TEST_DATABASE_URL, or run \`just setup-local\`" >&2
        echo "so .systemprompt/profiles/local/secrets.json carries a database_url." >&2
        exit 1
    fi
    printf 'export SYSTEMPROMPT_TEST_DATABASE_URL=%q\n' "$url"

# All tests. Every tier runs even after one fails: as `just` dependencies
# they stopped at the first red tier, so later tiers' failures stayed hidden
# behind earlier fixes for whole rounds.
test:
    #!/usr/bin/env bash
    set -uo pipefail
    failed=()
    for tier in test-unit test-integration test-contract test-e2e; do
        echo "==> $tier"
        {{just_executable()}} "$tier" || failed+=("$tier")
    done
    if [ "${#failed[@]}" -gt 0 ]; then printf '::error::test failed: %s\n' "${failed[@]}"; exit 1; fi

# End-to-end suite (Tier A): the FULL production API router in-process —
# gateway, per-role bridge manifest, Odoo sign-in with group→role mapping,
# and the real systemprompt-mcp-odoo binary over the MCP wire — against a
# throwaway database and a wiremock Odoo. Needs Docker Postgres (just db-up).
e2e:
    #!/usr/bin/env bash
    set -euo pipefail
    # dotenv-load pulls the real ODOO_URL/ODOO_DB from .env; the suite's Odoo
    # is a wiremock whose URL rides the fixture secrets, and env wins — unset.
    unset ODOO_URL ODOO_DB
    # These guards rebuild a MISSING server binary, never a STALE one. After a
    # core bump the old binaries still exist, still link against the previous
    # core, and fail to boot — the harness reports `never opened port NNNN —
    # check the profile it was spawned with`, which reads as a profile bug and
    # is not one. CI never sees this because CI builds fresh, which is exactly
    # why it costs an hour locally. After changing the core pin, rebuild the
    # MCP servers before trusting a red e2e run:
    #   cargo build -p systemprompt-mcp-agent -p systemprompt-mcp-odoo
    if [ ! -x target/release/systemprompt-mcp-odoo ] && [ ! -x target/debug/systemprompt-mcp-odoo ]; then
        echo "building systemprompt-mcp-odoo (the MCP wire test needs it)…"
        cargo build -p systemprompt-mcp-odoo
    fi
    if [ -z "${SYSTEMPROMPT_TEST_DATABASE_URL:-}" ] && [ -f .systemprompt/profiles/local/secrets.json ]; then
        SYSTEMPROMPT_TEST_DATABASE_URL=$(python3 -c "
    import json, urllib.parse as up
    u = up.urlsplit(json.load(open('.systemprompt/profiles/local/secrets.json'))['database_url'])
    print(up.urlunsplit((u.scheme, u.netloc, '/postgres', '', '')))")
        export SYSTEMPROMPT_TEST_DATABASE_URL
    fi
    cargo nextest run --manifest-path tests/Cargo.toml -p e2e-tests

# Tier A without the MCP subprocess build — the quickest full-router signal.
e2e-fast:
    #!/usr/bin/env bash
    set -euo pipefail
    unset ODOO_URL ODOO_DB
    if [ -z "${SYSTEMPROMPT_TEST_DATABASE_URL:-}" ] && [ -f .systemprompt/profiles/local/secrets.json ]; then
        SYSTEMPROMPT_TEST_DATABASE_URL=$(python3 -c "
    import json, urllib.parse as up
    u = up.urlsplit(json.load(open('.systemprompt/profiles/local/secrets.json'))['database_url'])
    print(up.urlunsplit((u.scheme, u.netloc, '/postgres', '', '')))")
        export SYSTEMPROMPT_TEST_DATABASE_URL
    fi
    cargo nextest run --manifest-path tests/Cargo.toml -p e2e-tests -E 'not test(a_signed_in_user)'

# End-to-end smoke (Tier B): drives the RUNNING local stack over real HTTP —
# seeds e2e-admin@/e2e-sales@ in Odoo, signs in as both, diffs their
# manifests, and runs the chatter tools through the MCP proxy. Reuses the
# running server and Odoo; never starts or restarts anything.
# Prereqs: `just start` and `just db-up local` + `just odoo-local-init`.
e2e-live:
    cargo nextest run --manifest-path tests/Cargo.toml -p e2e-tests --features live -E 'test(live_smoke)' --no-capture

# Seeds the /admin/demo dashboards with real telemetry for both roles: PKCE
# sign-in, an honestly minted hook token (bridge oauth-client →
# client_credentials, audience=hook), then hook sessions through
# /api/public/hooks/{track,govern} and small gateway calls so tokens land in
# the attribution window. Reuses the running server; never restarts anything.
# Prereqs: `just start`, `just db-up local`, `just odoo-local-init`.
e2e-live-demo-seed:
    @demo/skills/07-skill-usage-seed.sh

# Logged-in screenshots of the four Demo pages as an admin and as a non-admin,
# into playwright/demo-shots/{admin,user}/ (gitignored). Run
# `just e2e-live-demo-seed` first — the assertions read the rows it produces.
demo-shots PORT="8081":
    #!/usr/bin/env bash
    set -euo pipefail
    if [ ! -d playwright/node_modules ]; then
        echo "==> installing Playwright dependencies"
        just e2e-install
    fi
    cd playwright && GATEWAY_URL="${GATEWAY_URL:-http://localhost:{{PORT}}}" \
        npx playwright test tests/demo-dashboard.spec.ts
    echo "open playwright/demo-shots/"

# Reject tests that return early on a missing prerequisite without saying so
lint-silent-skips:
    ./scripts/lint-silent-skips.sh tests

# Source gates ported from systemprompt-core (scripts/*.sh)
lint-gates:
    @scripts/build-coordinator.sh run lint-gates "" -- {{just_executable()}} _lint-gates-uncoordinated

# Gates are independent read-only checks; they run concurrently and every
# failure is reported, so one red gate cannot hide the rest.
_lint-gates-uncoordinated:
    #!/usr/bin/env bash
    set -uo pipefail
    gates=(
        check-discarded-results.sh
        check-fail-open.sh
        lint-schema.sh
        lint-extensions.sh
        check-migration-numbers.sh
        lint-layers.sh
        lint-repo-construction.sh
        check-json-value.sh
        check-sqlx.sh
        check-http-errors.sh
        check-test-value.sh
        lint-silent-skips.sh
        lint-raw-ids.sh
        check-glob-reexports.sh
        check-comments.sh
        lint-inline-comments.sh
        check-duplicate-types.sh
        check-field-copy-from.sh
        check-repository-naming.sh
        check-web-transport.sh
        check-admin-template-links.sh
        check-admin-template-assets.sh
        # admin-css-classes + frontend-standards now run as cargo tests in
        # extensions/web/tests/ (admin_css_classes.rs, frontend_standards.rs).
        check-fork-drift.sh
        check-bridge-overlay-drift.sh
        check-dead-repository-code.sh
        check-file-headers.sh
        check-file-size.sh
        check-asset-reachability.sh
        check-workspace-deps.sh
        check-dockerfile-paths.sh
        check-dropped-schema.sh
        validate-services.sh
        check-mcp-tool-names.sh
        check-release-version.sh
        check-core-ref.sh
        check-schema-baseline.sh
        coverage-badge.sh
        check-docs-version.sh
        check-template-fields.sh
    )
    logdir=$(mktemp -d)
    trap 'rm -rf "$logdir"' EXIT
    pids=()
    for gate in "${gates[@]}"; do
        bash "scripts/$gate" >"$logdir/$gate.log" 2>&1 &
        pids+=("$!:$gate")
    done
    failed=()
    for entry in "${pids[@]}"; do
        pid=${entry%%:*}
        gate=${entry#*:}
        if ! wait "$pid"; then
            failed+=("$gate")
        fi
    done
    if [ ${#failed[@]} -gt 0 ]; then
        for gate in "${failed[@]}"; do
            echo "==== FAILED: $gate ===="
            cat "$logdir/$gate.log"
        done
        echo "lint gates failed: ${failed[*]}"
        exit 1
    fi
    echo "all ${#gates[@]} lint gates passed"

# The whole gate, in one command — exactly what .github/workflows/gates.yml
# runs on every push to next and on ordinary PRs (a frozen release PR reuses
# the next-push proof instead of re-running it). Run it before you push so CI
# is confirmation, not discovery. `preflight` adds the coverage floor/ratchet
# on top, which CI measures on main and nightly without blocking.
verify: preflight-static preflight-lint test
    @echo "verify: format, sqlx cache, lint gates, clippy, docs, msrv, and tests all pass"

# ══════════════════════════════════════════════════════════════════════════════
# PREFLIGHT (local stand-in for CI — tiered, cheapest first)
# ══════════════════════════════════════════════════════════════════════════════

# Everything: static gates → lint/doc/msrv → tests → coverage floor+ratchet.
preflight: preflight-static preflight-lint test coverage-check

# Tier 0 — seconds. Formatting, sqlx cache freshness, and the source gates.
# The gates run UNCOORDINATED here on purpose: they are read-only shell
# checks, so queueing them on the build lock only made a static run hang
# behind whoever was mid test run. `just lint-gates` stays coordinated for
# callers that want dedupe (clippy's dependency).
#
# Every check runs even after one fails, and the recipe fails if any did: a
# formatting slip must not hide the source gates until the next round.
preflight-static:
    #!/usr/bin/env bash
    set -uo pipefail
    failed=()
    check() { echo "==> $1"; shift; "$@" || failed+=("$*"); }
    check "fmt (root)" cargo fmt --all -- --check
    check "fmt (tests)" cargo fmt --manifest-path tests/Cargo.toml --all -- --check
    check "fmt (bridge)" cargo fmt --manifest-path bridge/Cargo.toml --all -- --check
    check "sqlx cache" bash scripts/check-sqlx-cache.sh
    check "core crate versions" bash scripts/check-core-crate-versions.sh
    check "release recipes present" bash -c '{{just_executable()}} --summary | tr " " "\n" | grep -qx deploy-release && {{just_executable()}} --summary | tr " " "\n" | grep -qx release'
    check "source gates" {{just_executable()}} _lint-gates-uncoordinated
    if [ "${#failed[@]}" -gt 0 ]; then printf '::error::preflight-static failed: %s\n' "${failed[@]}"; exit 1; fi

# Tier 1 — compilers. Clippy (all workspaces), rustdoc as errors, MSRV.
# Each runs even after one fails, as in preflight-static.
preflight-lint:
    #!/usr/bin/env bash
    set -uo pipefail
    failed=()
    for recipe in clippy doc-check msrv-check; do
        echo "==> $recipe"
        {{just_executable()}} "$recipe" || failed+=("$recipe")
    done
    if [ "${#failed[@]}" -gt 0 ]; then printf '::error::preflight-lint failed: %s\n' "${failed[@]}"; exit 1; fi

# Weekly deep pass: preflight plus the network-touching supply-chain gates.
preflight-full: preflight deny audit machete hack

# Rustdoc with warnings denied, across all three workspaces (root, tests/,
# bridge/) — mirrors core's quality.yml docs job. Single-flight coordinated.
doc-check:
    @scripts/build-coordinator.sh run doc-check "" -- {{just_executable()}} _doc-check-uncoordinated

_doc-check-uncoordinated:
    SQLX_OFFLINE=true RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
    SQLX_OFFLINE=true RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path tests/Cargo.toml --workspace --no-deps
    RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path bridge/Cargo.toml --no-deps

# Both workspaces must build on the declared minimum supported Rust version,
# and must declare the same one. The number is read from the manifests, never
# hardcoded — see scripts/check-msrv.sh for why that matters.
msrv-check:
    @scripts/build-coordinator.sh run msrv-check "" -- {{just_executable()}} _msrv-check-uncoordinated

_msrv-check-uncoordinated:
    bash scripts/check-msrv.sh

# ══════════════════════════════════════════════════════════════════════════════
# COVERAGE (raw llvm-cov; floor + ratchet vs tracked coverage/baseline.json)
# ══════════════════════════════════════════════════════════════════════════════

# Instrumented test run over all three workspaces; writes coverage-report/.
# See scripts/coverage.sh for the sccache/mold neutralisation notes.
coverage:
    @scripts/build-coordinator.sh run coverage "" -- bash scripts/coverage.sh

# Enforce the floor and ratchet recorded in coverage/baseline.json.
coverage-check: coverage
    bash scripts/coverage-check.sh

# Re-record coverage/baseline.json at the measured value (deliberate act —
# commit the result). Raise the "floor" field by hand as milestones land.
coverage-baseline: coverage
    UPDATE_BASELINE=1 bash scripts/coverage-check.sh

# Rewrite the README's coverage badge from coverage/baseline.json. Run it
# after `just coverage-baseline`; the `coverage-badge.sh --check` gate fails
# the build if the two disagree.
coverage-badge:
    bash scripts/coverage-badge.sh --write

# Record tests/fixtures/schema/release-baseline-<version>.sql: the schema a
# fresh install of a release produces, plus its extension_migrations rows,
# dumped by the local Postgres container's own pg_dump (always the server's
# major — the host client may be older and refuse). With no argument the
# current tree is installed under the workspace version — run it after every
# version bump (scripts/check-schema-baseline.sh enforces that). With a
# version, that release's server tarball is fetched and ITS binary does the
# install, so a rung can be added for a release that shipped before the
# ladder existed (Linux x86_64/arm64 hosts only; the gateway tarballs are
# built there). The upgrade test restores every rung into an empty database
# and runs the current installer over it; the ladder is append-only.
schema-baseline VERSION="":
    #!/usr/bin/env bash
    set -euo pipefail
    version="{{VERSION}}"
    tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
    if [ -n "$version" ]; then
        [ "$(uname -s)" = "Linux" ] || { echo "schema-baseline: release tarballs are only run on Linux here" >&2; exit 1; }
        case "$(uname -m)" in x86_64) arch=amd64 ;; aarch64|arm64) arch=arm64 ;; *) echo "schema-baseline: no tarball for $(uname -m)" >&2; exit 1 ;; esac
        name="systemprompt-internal-$version-linux-$arch.tar.gz"
        echo "schema-baseline: downloading $name"
        gh release download "v$version" -R systempromptio/systemprompt-internal -p "$name" -p SHA256SUMS -D "$tmp"
        (cd "$tmp" && grep " $name\$" SHA256SUMS | sha256sum -c - >/dev/null)
        tar xzf "$tmp/$name" -C "$tmp"
        cli="$tmp/${name%.tar.gz}/bin/systemprompt"
        core_ref="v$version"
    else
        just build
        version="$(awk '/^\[workspace\.package\]/{p=1;next}/^\[/{p=0}p&&/^version[[:space:]]*=/{gsub(/[[:space:]"]/,""); sub(/^version=/,""); print; exit}' Cargo.toml)"
        core_ref="$(tr -d '[:space:]' < bridge/CORE_REF)"
        cli="{{CLI}}"
    fi
    container="$(docker compose -p "$(just _project_name local)" -f .systemprompt/docker/local.yaml ps -q postgres)"
    [ -n "$container" ] || { echo "schema-baseline: local Postgres is not running (just db-up)" >&2; exit 1; }
    base_url="$(jq -r '.database_url' .systemprompt/profiles/local/secrets.json)"
    scratch="sp_schema_baseline_$$"
    scratch_url="${base_url%/*}/$scratch"
    fixture="tests/fixtures/schema/release-baseline-$version.sql"
    mkdir -p "$(dirname "$fixture")"
    cleanup() { docker exec "$container" psql -U systemprompt -d postgres -qc "DROP DATABASE IF EXISTS \"$scratch\" WITH (FORCE)" >/dev/null 2>&1 || true; rm -rf "$tmp"; }
    trap cleanup EXIT
    docker exec "$container" psql -U systemprompt -d postgres -qc "CREATE DATABASE \"$scratch\""
    echo "schema-baseline: fresh install of $version (core $core_ref) into $scratch"
    if ! log="$(SYSTEMPROMPT_DATABASE_URL="$scratch_url" "$cli" infra db migrate --profile local 2>&1)"; then
        echo "$log" | tail -30; echo "schema-baseline: fresh install failed" >&2; exit 1
    fi
    {
        echo "-- systemprompt-internal release-baseline: $version (core $core_ref)"
        echo "-- Recorded by 'just schema-baseline' from a fresh install; the upgrade test"
        echo "-- restores it and migrates forward. Re-record after every version bump."
        docker exec "$container" pg_dump -U systemprompt --schema-only --no-owner --no-privileges --no-comments "$scratch"
        docker exec "$container" pg_dump -U systemprompt --data-only --inserts --no-owner --table=extension_migrations "$scratch"
    } | grep -v -e '^\\' -e "set_config('search_path'" > "$fixture"
    echo "schema-baseline: wrote $fixture ($(wc -l < "$fixture") lines)"
    git diff --stat -- "$fixture" | tail -1
    bash scripts/check-schema-baseline.sh

# Browsable HTML tree from the last `just coverage` run.
coverage-html:
    #!/usr/bin/env bash
    set -euo pipefail
    ROOT="$(pwd)"
    if [ ! -f "$ROOT/coverage-report/tests.profdata" ]; then
        echo "Run 'just coverage' first" >&2
        exit 1
    fi
    TOOLDIR="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/^host: //p')/bin"
    TBASE="${COVERAGE_TARGET_DIR:-$ROOT/coverage-report/target}"
    BINS=$(for t in "$TBASE-root" "$TBASE-tests" "$TBASE-bridge"; do \
        find "$t/debug/deps" -maxdepth 1 -executable -type f ! -name '*.d' ! -name '*.so' -printf '%T@ %p\n' 2>/dev/null; \
    done | sort -rn | awk '{ base=$2; sub(".*/", "", base); sub(/-[0-9a-f]+$/, "", base); if (!seen[base]++) print $2 }')
    OBJ_ARGS=""
    for b in $BINS; do OBJ_ARGS="$OBJ_ARGS --object $b"; done
    mkdir -p "$ROOT/coverage-report/html"
    "$TOOLDIR/llvm-cov" show \
        --instr-profile="$ROOT/coverage-report/tests.profdata" \
        $OBJ_ARGS \
        --ignore-filename-regex="(\.cargo|/rustc/|/registry/|/debug/build/|/tests/|/target/|systemprompt-core/|systemprompt-internal/src/(main|lib)\.rs|bridge/src/main\.rs|extensions/.*/extension\.rs|build\.rs)" \
        --format=html \
        --output-dir="$ROOT/coverage-report/html"
    echo "Coverage report: coverage-report/html/index.html"

# Remove all coverage artifacts (instrumented target dirs included).
# Refuses while a coordinated run holds the lock: coverage-report/ carries the
# instrumented test binaries, so deleting it mid-run makes every remaining test
# fail to exec ("No such file or directory", nextest exit 70) and the report
# come out at 0.00% — a failure that looks like a code regression and is not.
coverage-clean:
    #!/usr/bin/env bash
    set -euo pipefail
    LOCK="${COORD_STATE_DIR:-{{ justfile_directory() }}/.build}/lock"
    if [ -d "$LOCK" ]; then
        PID="$(cat "$LOCK/pid" 2>/dev/null || echo)"
        if [ -n "$PID" ] && kill -0 "$PID" 2>/dev/null; then
            echo "refusing: '$(cat "$LOCK/recipe" 2>/dev/null || echo run)' is running (pid $PID)." >&2
            echo "  Deleting coverage-report/ now would pull the instrumented binaries" >&2
            echo "  out from under it. Wait for it, or override with COVERAGE_CLEAN_FORCE=1." >&2
            [ "${COVERAGE_CLEAN_FORCE:-0}" = "1" ] || exit 1
        fi
    fi
    rm -rf coverage-report/

# Point git at the tracked hooks (pre-commit patch-marker guard + fast
# gates). There is deliberately NO pre-push hook: pushes to next are gated by
# CI (gates.yml), and `just verify` is run by hand before pushing. Run once
# per clone.
init-hooks:
    git config core.hooksPath .githooks
    @echo "git hooks now sourced from .githooks/"

# Cross-file referential integrity for services/ (ACL entity ids, MCP ports)
validate:
    bash scripts/validate-services.sh

# Shared sources that differ from the sibling fork must be recorded in
# .fork-divergence. Needs SIBLING_REPO; skips cleanly without it.
check-fork-drift:
    bash scripts/check-fork-drift.sh

# Verify every production extension source has a `//!` module head
check-headers:
    bash scripts/check-file-headers.sh

# Observational Rust-standards audit — appends to ISSUE.md, never blocks
audit-standards:
    bash scripts/audit-rust-standards.sh

# 300-line ceiling on extension sources (same script CI runs)
file-size:
    bash scripts/check-file-size.sh

# Every Cargo workspace in the repo. `tests/` and `bridge/` are excluded from
# the root workspace, so a bare root-level scan silently skips their lockfiles.
# Keep in sync with `git ls-files '*Cargo.lock'`.
workspaces := ". tests bridge"

# Detect unused dependencies across every workspace
machete:
    #!/usr/bin/env bash
    set -euo pipefail
    for w in {{ workspaces }}; do
        echo "==> cargo machete: $w"
        (cd "$w" && cargo machete)
    done

# Supply-chain gates across every workspace: cargo-deny (licenses/bans/
# advisories, root deny.toml discovered via --manifest-path) and cargo-audit
deny:
    #!/usr/bin/env bash
    set -euo pipefail
    for w in {{ workspaces }}; do
        echo "==> cargo deny: $w"
        cargo deny --manifest-path "${w%/}/Cargo.toml" check
    done

check-bans:
    cargo deny check bans

audit:
    #!/usr/bin/env bash
    set -euo pipefail
    for w in {{ workspaces }}; do
        echo "==> cargo audit: $w"
        cargo audit --file "${w%/}/Cargo.lock"
    done

# Build every feature powerset (catches feature-flag drift); weekly tier only
hack:
    cargo hack --workspace --feature-powerset --depth 2 check

# Structural guard: `UserId::admin()` is banned outside sanctioned call sites.
# The allowlist is empty by design — this repo has no sanctioned site; adding
# one requires justification in review.
lint-no-untyped-admin:
    #!/usr/bin/env bash
    set -euo pipefail
    hits=$(grep -rn 'UserId::admin()' extensions/ src/ bridge/src/ --include='*.rs' 2>/dev/null \
        | grep -v '/tests/' \
        || true)
    if [ -n "$hits" ]; then
        echo "lint-no-untyped-admin: untyped UserId::admin() outside the sanctioned call sites:"
        echo "$hits"
        exit 1
    fi

# Structural guard: no string-literal `UserId::new("...")` in extension code.
# String literals are how principal synthesis sneaks in — every legitimate
# UserId::new call takes a validated identifier as a variable, never a literal.
# Allowlisted: test code (regression tests intentionally construct ids) and
# any future bootstrap/provisioning module.
lint-no-synthesis:
    #!/usr/bin/env bash
    set -euo pipefail
    hits=$(grep -rEn 'UserId::new\("' extensions/ \
        --include='*.rs' \
        --exclude-dir=tests \
        --exclude-dir=bootstrap \
        || true)
    if [ -n "$hits" ]; then
        echo "error: forbidden synthesized principal — UserId::new with string literal"
        echo "$hits"
        echo
        echo "UserId::new must take a validated identifier (from cookie, query,"
        echo "JWT claim, or DB row), never a hard-coded literal. If this is"
        echo "legitimate bootstrap code, move it to extensions/**/bootstrap/."
        exit 1
    fi

# Prepare SQLx offline query cache (requires running database)
prepare:
    scripts/sqlx-prepare.sh "{{CLI}}"

# ══════════════════════════════════════════════════════════════════════════════
# SERVICES & DATABASE
# ══════════════════════════════════════════════════════════════════════════════

# Start server (always uses local profile)
start:
    {{CLI}} infra services start --profile local

# Optional: running server + binary provenance + recent build/lint/test results
[unix]
server-status:
    @scripts/server-state.sh report

# Start server with release binary
start-release:
    {{CLI_RELEASE}} infra services start --profile local

# Stop this clone's services
stop:
    {{CLI}} infra services stop --all

# Run migrations
migrate:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -z "${SYSTEMPROMPT_PROFILE:-}" ]; then
        export SYSTEMPROMPT_PROFILE="{{justfile_directory()}}/.systemprompt/profiles/local/profile.yaml"
    fi
    {{CLI}} infra db migrate

# When an already-applied migration file is edited (e.g. a seed fix), its
# stored checksum stops matching the file and `migrate` / `start` refuse to
# proceed. `infra db migrate-repair` re-aligns the tracking table by dropping
# the drifted rows and re-applying those migrations — every migration is
# idempotent (guarded seeds or CREATE ... IF NOT EXISTS), so re-running them
# re-records the current checksum without touching your data.
# Repair migration checksum drift in place — no data loss, no destructive reset.
repair-migrations:
    {{CLI}} infra db migrate-repair --apply

# Per-clone docker compose project name. Derived from the absolute justfile directory
# so a second clone on the same host gets its own containers and volumes.
_project_name TENANT:
    #!/usr/bin/env bash
    set -euo pipefail
    HASH=$(printf '%s' "{{justfile_directory()}}" | { sha256sum 2>/dev/null || shasum -a 256; } | cut -c1-8)
    LEAF=$(basename "{{justfile_directory()}}" | tr '_' '-' | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9-]/-/g')
    printf 'sp-%s-%s-%s\n' "$LEAF" "$HASH" "{{TENANT}}"

# Start PostgreSQL for a specific tenant (default: local)
db-up TENANT="local":
    docker compose -p "$(just _project_name {{TENANT}})" -f .systemprompt/docker/{{TENANT}}.yaml up -d

# Stop PostgreSQL for a specific tenant
db-down TENANT="local":
    docker compose -p "$(just _project_name {{TENANT}})" -f .systemprompt/docker/{{TENANT}}.yaml down

# Show PostgreSQL logs for a specific tenant
db-logs TENANT="local":
    docker compose -p "$(just _project_name {{TENANT}})" -f .systemprompt/docker/{{TENANT}}.yaml logs -f

# List all tenant databases
db-list:
    @ls -1 .systemprompt/docker/*.yaml 2>/dev/null | xargs -I {} basename {} .yaml || echo "No tenant databases found"

# ══════════════════════════════════════════════════════════════════════════════
# AUTH & TENANT & PROFILE
# ══════════════════════════════════════════════════════════════════════════════

# Authenticate with SystemPrompt Cloud
login ENV="production":
    {{CLI}} cloud auth login {{ENV}}

# Clear saved credentials
logout:
    {{CLI}} cloud auth logout

# Show current user and tenant
whoami:
    {{CLI}} cloud auth whoami

# Tenant operations (interactive menu)
tenant:
    {{CLI}} cloud tenant

# Set up a local-only profile + Docker Postgres (no cloud, no login required).
# Pass keys as positional args, or leave blank to be prompted interactively:
#   just setup-local sk-ant-... sk-... AIza...
# Port, Postgres port and Odoo port can be overridden for running multiple clones on one host:
#   just setup-local sk-ant-... "" "" 8081 5433 8071
setup-local ANTHROPIC_KEY="" OPENAI_KEY="" GEMINI_KEY="" HTTP_PORT="8080" PG_PORT="5432" ODOO_PORT="8070":
    #!/usr/bin/env bash
    set -euo pipefail
    ROOT="{{justfile_directory()}}"
    PROFILE_DIR="$ROOT/.systemprompt/profiles/local"
    DOCKER_DIR="$ROOT/.systemprompt/docker"
    ANTHROPIC_KEY="{{ANTHROPIC_KEY}}"
    OPENAI_KEY="{{OPENAI_KEY}}"
    GEMINI_KEY="{{GEMINI_KEY}}"
    HTTP_PORT="{{HTTP_PORT}}"
    PG_PORT="{{PG_PORT}}"
    ODOO_PORT="{{ODOO_PORT}}"
    export SYSTEMPROMPT_PROFILE="$PROFILE_DIR/profile.yaml"
    # Whether a key was passed as a positional arg. When none is and there is
    # nothing to preserve, generation still needs a provider: on a TTY we let
    # `admin setup` drive its own "Select your AI provider" menu (the CLI owns
    # the prompt); off a TTY we cannot prompt, so keys must come as args. A
    # developer who keeps .systemprompt/ across reclones re-runs with no args
    # and is never asked again (the profile.yaml guard below skips generation).
    HAS_KEY=false
    if [ -n "$ANTHROPIC_KEY" ] || [ -n "$OPENAI_KEY" ] || [ -n "$GEMINI_KEY" ]; then
        HAS_KEY=true
    fi
    if [ "$HAS_KEY" = false ] && [ ! -f "$PROFILE_DIR/secrets.json" ] && [ ! -t 0 ]; then
        echo ""
        echo "================================================================"
        echo "  setup-local needs an AI provider API key"
        echo "================================================================"
        echo ""
        echo "  Not running on a TTY, so the provider menu can't be shown."
        echo "  Pass a key as an argument (one of Anthropic, OpenAI, Gemini):"
        echo "    just setup-local <anthropic_key> [openai_key] [gemini_key]"
        echo ""
        exit 1
    fi
    if [ ! -x target/debug/systemprompt ] && [ ! -x target/release/systemprompt ]; then
        echo "Building debug binary..."
        just build
    fi
    # Resolve the binary at runtime: the {{CLI}} variable is evaluated by `just`
    # at parse time, so on a cold clone (no binary yet) it expands to an error
    # stub — useless for the bootstrap/keygen calls below, which run only after
    # the build above has produced the binary.
    if [ -x target/release/systemprompt ]; then
        BIN="$ROOT/target/release/systemprompt"
    else
        BIN="$ROOT/target/debug/systemprompt"
    fi
    mkdir -p "$PROFILE_DIR" "$DOCKER_DIR"
    if [ ! -f "$DOCKER_DIR/local.yaml" ]; then
        echo "Writing Docker compose for local Postgres (host port $PG_PORT) + Odoo (host port $ODOO_PORT)..."
        cat > "$DOCKER_DIR/local.yaml" <<YAML
    services:
      postgres:
        image: postgres:18-alpine
        restart: unless-stopped
        environment:
          POSTGRES_USER: systemprompt
          POSTGRES_PASSWORD: 123
          POSTGRES_DB: systemprompt
        ports:
          - "${PG_PORT}:5432"
        volumes:
          - postgres_data:/var/lib/postgresql
        healthcheck:
          test: ["CMD-SHELL", "pg_isready -U systemprompt -d systemprompt"]
          interval: 5s
          timeout: 5s
          retries: 5
      # Local Odoo CE mirroring the production companion app (deploy/fly/odoo/).
      # Needs the odoo role + odoo_local DB once: \`just odoo-local-init\` (after db-up).
      odoo:
        image: odoo:18
        restart: unless-stopped
        depends_on:
          postgres:
            condition: service_healthy
        environment:
          HOST: postgres
          USER: odoo
          PASSWORD: odoo
        ports:
          - "${ODOO_PORT}:8069"
        volumes:
          - odoo_web_data:/var/lib/odoo
        healthcheck:
          test: ["CMD", "curl", "-fsS", "http://localhost:8069/web/health"]
          interval: 10s
          timeout: 5s
          retries: 12
    volumes:
      postgres_data: {}
      odoo_web_data: {}
    YAML
    fi
    # Seed the Odoo connection into the profile's secrets.json as custom keys.
    # SecretsBootstrap carries them in-process, and the MCP spawner uppercases
    # custom keys into the subprocess env (odoo_url → ODOO_URL), so nothing
    # needs a .env file. Runs after `admin setup` below has written the file.
    seed_odoo_secrets() {
        local secrets_file="$PROFILE_DIR/secrets.json"
        [ -f "$secrets_file" ] || return 0
        ODOO_PORT="$ODOO_PORT" python3 - "$secrets_file" <<'PY'
    import json, os, sys
    path = sys.argv[1]
    data = json.load(open(path))
    if "odoo_url" not in data:
        data["odoo_url"] = f"http://localhost:{os.environ['ODOO_PORT']}"
        data["odoo_db"] = data.get("odoo_db", "odoo_local")
        json.dump(data, open(path, "w"), indent=2)
        print("Seeded odoo_url / odoo_db into profile secrets (local Odoo sidecar).")
    PY
    }
    if [ ! -f "$PROFILE_DIR/profile.yaml" ]; then
        echo "Generating profile + provider registry + secrets via 'admin setup'..."
        if [ "$HAS_KEY" = true ]; then
            # Keys supplied as args: fully non-interactive. The default provider
            # is the first key given, so the generated config (the providers
            # registry, gateway default, ai/config.yaml) is consistent with the
            # single key.
            KEY_ARGS=()
            DEFAULT_PROVIDER=""
            if [ -n "$ANTHROPIC_KEY" ]; then KEY_ARGS+=(--anthropic-key "$ANTHROPIC_KEY"); [ -z "$DEFAULT_PROVIDER" ] && DEFAULT_PROVIDER=anthropic; fi
            if [ -n "$OPENAI_KEY" ]; then KEY_ARGS+=(--openai-key "$OPENAI_KEY"); [ -z "$DEFAULT_PROVIDER" ] && DEFAULT_PROVIDER=openai; fi
            if [ -n "$GEMINI_KEY" ]; then KEY_ARGS+=(--gemini-key "$GEMINI_KEY"); [ -z "$DEFAULT_PROVIDER" ] && DEFAULT_PROVIDER=gemini; fi
            "$BIN" admin setup --yes --no-migrate --environment local \
                --db-host localhost --db-port "$PG_PORT" \
                --db-user systemprompt --db-password 123 --db-name systemprompt \
                --default-provider "$DEFAULT_PROVIDER" \
                "${KEY_ARGS[@]}"
        else
            # No key arg: let the CLI prompt for which provider to use. DB,
            # environment, and migrations stay non-interactive (flags + env);
            # only the provider selection is interactive, and the chosen
            # provider becomes the default.
            SYSTEMPROMPT_NON_INTERACTIVE=1 "$BIN" admin setup --no-migrate --environment local \
                --db-host localhost --db-port "$PG_PORT" \
                --db-user systemprompt --db-password 123 --db-name systemprompt
        fi
        if [ "$HTTP_PORT" != "8080" ]; then
            "$BIN" admin config server set --port "$HTTP_PORT" \
                --api-server-url "http://localhost:${HTTP_PORT}" \
                --api-internal-url "http://localhost:${HTTP_PORT}" \
                --api-external-url "http://localhost:${HTTP_PORT}"
            # The authz hook URL is an absolute webhook target baked at
            # `admin setup` time on the default port; re-point it at the
            # chosen port so the gateway's govern callback reaches this server.
            "$BIN" admin config governance set --mode webhook \
                --url "http://localhost:${HTTP_PORT}/api/public/govern/authz"
            # jwt_issuer is baked on the default port too, and it is not
            # cosmetic: it is the base a client resolves
            # `{iss}/.well-known/jwks.json` against. Left at :8080 on a
            # non-default port, every external verifier (the bridge, Claude
            # Code) fetches the signing keys of whatever else is on :8080 —
            # or nothing — and rejects this server's tokens as minted under an
            # unknown authority.
            "$BIN" admin config security set \
                --jwt-issuer "http://localhost:${HTTP_PORT}"
            # Same for CORS: the seeded origins name :8080, so the admin UI
            # served from the chosen port is refused by its own API.
            "$BIN" admin config server cors add "http://localhost:${HTTP_PORT}" || true
            "$BIN" admin config server cors add "http://127.0.0.1:${HTTP_PORT}" || true
            "$BIN" admin config server cors remove "http://localhost:8080" || true
            "$BIN" admin config server cors remove "http://127.0.0.1:8080" || true
        fi
    elif [ "$HAS_KEY" = true ]; then
        # Profile generation is one-shot, guarded on profile.yaml. `just db-down`
        # drops the database but leaves the profile, so a re-run with different
        # keys would silently keep the old provider registry. Say so loudly and
        # point at the one command that actually re-provisions.
        echo ""
        echo "================================================================"
        echo "  Existing profile reused — supplied keys were NOT applied"
        echo "================================================================"
        echo ""
        echo "  $PROFILE_DIR/profile.yaml already exists, so 'admin setup' was"
        echo "  skipped and the provider registry/keys are unchanged."
        echo "  To re-provision from the keys you just passed:"
        echo ""
        echo "    rm -rf \"$PROFILE_DIR\" && just setup-local <keys...> $HTTP_PORT $PG_PORT"
        echo ""
    fi
    seed_odoo_secrets
    mkdir -p "$ROOT/web/dist"
    echo "Building binaries (release, full workspace)..."
    just build --release
    echo "Starting local Postgres via Docker..."
    just db-up local
    echo "Waiting for Postgres to accept connections on localhost:${PG_PORT}..."
    for i in $(seq 1 60); do
        if (exec 3<>/dev/tcp/127.0.0.1/${PG_PORT}) 2>/dev/null; then
            exec 3<&- 3>&-
            # Also confirm the server actually answers pg_isready, not just a half-open socket.
            CONTAINER=$(docker compose -p "$(just _project_name local)" -f .systemprompt/docker/local.yaml ps -q postgres)
            if [ -n "$CONTAINER" ] && docker exec "$CONTAINER" pg_isready -U systemprompt -d systemprompt >/dev/null 2>&1; then
                echo "Postgres is ready."
                break
            fi
        fi
        if [ "$i" = "60" ]; then
            echo "ERROR: Postgres did not become ready within 60s." >&2
            exit 1
        fi
        sleep 1
    done
    echo "Running database migrations..."
    just migrate
    echo "Ensuring bootstrap admin user..."
    "$BIN" admin bootstrap
    if [ ! -f "$ROOT/signing_key.pem" ]; then
        echo "Generating JWT signing key..."
        "$BIN" admin keys generate --output "$ROOT/signing_key.pem"
    fi
    echo "Publishing assets..."
    just publish
    echo ""
    echo "Local setup complete. Run: just start"

# List all tenants
tenants:
    {{CLI}} cloud tenant list

# Profile operations (interactive menu)
profile:
    {{CLI}} cloud profile

# List all profiles
profiles:
    {{CLI}} cloud profile list

# ══════════════════════════════════════════════════════════════════════════════
# SYNC
# ══════════════════════════════════════════════════════════════════════════════

# Content and skills are ingested from services/ at server startup and by
# `just publish` (publish_pipeline job); there is no separate local sync command.

# Core 0.29.0 removed `cloud sync`. Pushing is `just deploy` (cloud deploy),
# and pulling is `cloud backup`, which downloads the tenant's runtime services/
# tree. The old sync-push / sync-pull recipes called a command that no longer
# exists, so they are gone rather than aliased to something they never were.

# Download the tenant's runtime services/ tree (--list to inspect first)
backup *ARGS:
    {{CLI}} cloud backup "$@"

# ══════════════════════════════════════════════════════════════════════════════
# DEPLOY
# ══════════════════════════════════════════════════════════════════════════════

# Build everything and deploy to cloud — one command, no preceding build step.
# Note: publish_pipeline runs automatically on server startup with correct profile URLs
# Pinned to the `production` profile so a deploy never follows whichever profile
# the CLI session happens to be switched to.
#
# Warns on a dirty working tree (but proceeds): the image and the synced
# services/ tree are built from what is on disk, so uncommitted state ships to
# production. The warning lists what is going out so a half-committed deploy
# is at least a visible act, not a silent one.
deploy *FLAGS: core-guard _docker-preflight build-all deploy-check
    @if [ -n "$(git status --porcelain)" ]; then \
        echo "WARNING: working tree is dirty — this deploy ships the uncommitted state below:"; \
        git status --porcelain | head -20; \
    fi
    PATH="$(scripts/docker-path.sh)" {{CLI_RELEASE}} cloud deploy --profile {{DEPLOY_PROFILE}} {{FLAGS}}

# `cloud deploy` shells out to `docker build`. A wrapper shim ahead of the real
# binary on PATH (0.51.0 hit one) fails the build with an error that names
# neither docker nor the shim, so pin /usr/bin first when the real binary is
# there and name whatever else `docker` resolves to. `scripts/docker-path.sh`
# prints the pinned PATH so the deploy step itself runs under it.
_docker-preflight:
    #!/usr/bin/env bash
    set -euo pipefail
    export PATH="$(scripts/docker-path.sh)"
    resolved="$(command -v docker || true)"
    if [ -z "$resolved" ]; then
        echo "ERROR: docker not found on PATH — cloud deploy needs 'docker build'"
        exit 1
    fi
    case "$resolved" in
        /usr/bin/*) ;;
        *) echo "warning: docker resolves to $resolved, not /usr/bin/docker — a wrapper shim can fail 'docker build' with an unrelated error" ;;
    esac

# Deploy the NEXT stack to production (internal.systemprompt.io) as a
# PARALLEL process: a dedicated git worktree of origin/next with its own
# target/ and build coordinator, so it never contends with this clone's
# builds and never touches `just deploy` (the main/crates.io release act).
# The gitignored .systemprompt/ (profiles, secrets, Dockerfile) is synced in
# because a worktree only carries tracked files.
deploy-next *FLAGS: _docker-preflight
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{justfile_directory()}}"
    dir="$(cd "$root/.." && pwd)/systemprompt-internal-deploy-next"
    git -C "$root" fetch origin next
    if [ ! -d "$dir" ]; then
        git -C "$root" worktree add --detach "$dir" origin/next
    else
        git -C "$dir" checkout --detach origin/next
    fi
    rsync -a --delete "$root/.systemprompt/" "$dir/.systemprompt/"
    # The local profile carries absolute paths into THIS clone; left alone,
    # the worktree's asset jobs would write web/dist into the wrong tree and
    # the image build would find nothing to copy. Production's paths are
    # container paths (/app) and are untouched.
    sed -i "s|$root|$dir|g" "$dir/.systemprompt/profiles/local/profile.yaml"
    echo "deploy-next: worktree at $dir on $(git -C "$dir" rev-parse --short HEAD)"
    cd "$dir" && just deploy {{FLAGS}}

# What "build next and next together" means in practice: while
# [patch.crates-io] is active the server compiles against ../systemprompt-core
# in place (no vendored copy), so the only way a deploy can ship exactly the
# core CI will gate is if that checkout is clean and sits at bridge/CORE_REF.
# Refuses otherwise; with the patch dormant there is nothing to check.
core-guard:
    #!/usr/bin/env bash
    set -euo pipefail
    grep -qE '^\[patch\.crates-io\]' Cargo.toml || { echo "core-guard: patch dormant; building against the published core"; exit 0; }
    core="${CORE_REPO:-../systemprompt-core}"
    # -e, not -d: a worktree's .git is a file, and a detached worktree is the sanctioned clean checkout.
    [ -e "$core/.git" ] || { echo "core-guard: no core checkout at $core" >&2; exit 1; }
    expected="$(tr -d '[:space:]' < bridge/CORE_REF)"
    head="$(git -C "$core" rev-parse HEAD)"
    pinned="$(git -C "$core" rev-parse "$expected^{commit}" 2>/dev/null || echo "$expected")"
    [ "$head" = "$pinned" ] || { echo "core-guard: $core is at ${head:0:12}, bridge/CORE_REF pins $expected; commit and 'just core-pin' first" >&2; exit 1; }
    dirty="$(git -C "$core" status --porcelain --untracked-files=all)"
    [ -z "$dirty" ] || { echo "core-guard: $core has uncommitted changes; commit them on core next (or set them aside) before deploying:" >&2; echo "$dirty" >&2; exit 1; }
    echo "core-guard: $core clean at ${head:0:12} == bridge/CORE_REF"

# Pre-deploy preflight — no build, no push. `deploy` depends on it, so a
# production profile the binary would refuse to boot (no server.instance_id,
# missing identity secrets) is caught here, not after the image is live.
# The pre-deploy gate. It runs the RELEASE binary on purpose: that is the one
# `deploy` is about to ship, so doctoring anything else proves nothing. It also
# has to be this way — `CLI` picks its path from path_exists at justfile load
# time, so in a tree with no binary yet it is already frozen to an error string
# by the time `build-all` has produced one. That is why `deploy` builds first
# and checks second; the reverse order could never pass on a fresh worktree
# (which is exactly what `deploy-next` creates the first time it runs).
deploy-check:
    @if [ ! -x "{{CLI_RELEASE}}" ]; then \
        echo "ERROR: no release binary at $(pwd)/{{CLI_RELEASE}}"; \
        echo "       run 'just build --release' IN THIS TREE first — note the path above;"; \
        echo "       a binary in another clone or worktree does not count."; \
        exit 1; \
    fi
    {{CLI_RELEASE}} cloud doctor --profile {{DEPLOY_PROFILE}} --distributed

# Check deployment status
status:
    {{CLI}} cloud status --profile {{DEPLOY_PROFILE}}

# ══════════════════════════════════════════════════════════════════════════════
# MCP & BUILD ALL
# ══════════════════════════════════════════════════════════════════════════════

# Build all MCP servers (reads from manifest.yaml files) — single-flight and
# fingerprint-skipped, so a tree whose MCP servers already built returns
# immediately instead of re-paying the per-package feature-unification rebuild.
build-mcp:
    @scripts/build-coordinator.sh run build-mcp "" -- {{just_executable()}} _build-mcp-uncoordinated

_build-mcp-uncoordinated:
    DATABASE_URL="$(just _db-url)" {{CLI}} build mcp --release

# Build everything for deployment (Rust binary + MCP servers + web assets)
# The bridge is NOT built here: it ships from the GitHub release that
# .github/workflows/release.yml cuts on every merge to main, and the admin
# pages link to that release by version.
build-all:
    just build --release
    just build-mcp
    just web-build
    {{CLI_RELEASE}} infra jobs run publish_pipeline --profile local
    @echo "All components built"

# ══════════════════════════════════════════════════════════════════════════════
# WEB ASSETS & PUBLISHING
# ══════════════════════════════════════════════════════════════════════════════

# Copy web assets to dist (CSS, JS, images)
web-assets:
    {{CLI}} infra jobs run copy_extension_assets

# Publish: compile templates, bundle CSS/JS, copy assets, prerender content
publish:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -z "${SYSTEMPROMPT_PROFILE:-}" ]; then
        export SYSTEMPROMPT_PROFILE="{{justfile_directory()}}/.systemprompt/profiles/local/profile.yaml"
    fi
    {{CLI}} infra jobs run publish_pipeline

# Build web assets only (templates + CSS + JS + copy to dist)
web-build:
    {{CLI}} infra jobs run bundle_admin_css
    {{CLI}} infra jobs run copy_extension_assets

# ══════════════════════════════════════════════════════════════════════════════
# DOCKER
# ══════════════════════════════════════════════════════════════════════════════

# Build Docker image for local testing
docker-build TAG="local":
    docker build -f Dockerfile -t systemprompt-internal:{{TAG}} .

# Run image locally for testing
docker-run TAG="local":
    docker run -p 8080:8080 --env-file .env systemprompt-internal:{{TAG}}

# Build the branded bridge. Its own standalone workspace, NOT the server
# workspace — `just build` does not touch it, and a bare `cargo build` from the
# repo root silently builds the server instead.
bridge-build *ARGS: (core-checkout "warn")
    cd {{justfile_directory()}}/bridge && cargo build --release {{ARGS}}

# The client depends on systemprompt-bridge by relative path, and that crate is
# not published — so unlike the server, building the client needs the core
# repository checked out beside this one. Clones it when absent, leaves local
# work alone, and compares a clean checkout with bridge/CORE_REF: a mismatch
# is fatal by default and a warning with MISMATCH=warn. It never moves the
# checkout — fast-forwarding here once mixed a server built from one core
# with a bridge built from another.
core-checkout MISMATCH="fail":
    #!/usr/bin/env bash
    set -euo pipefail
    CORE="{{justfile_directory()}}/../systemprompt-core"
    if [ -e "$CORE/.git" ]; then
        if [ -n "$(git -C "$CORE" status --porcelain)" ]; then
            echo "core checkout has local changes — leaving it as it is."
        else
            echo "Using existing core checkout at $(git -C "$CORE" rev-parse --short HEAD)"
            expected="$(tr -d '\r\n' < "{{justfile_directory()}}/bridge/CORE_REF")"
            head="$(git -C "$CORE" rev-parse HEAD)"
            pinned="$(git -C "$CORE" rev-parse "$expected^{commit}" 2>/dev/null || true)"
            if [ -z "$pinned" ]; then
                git -C "$CORE" fetch --quiet --tags origin || true
                pinned="$(git -C "$CORE" rev-parse "$expected^{commit}" 2>/dev/null || echo "$expected")"
            fi
            if [ "$head" != "$pinned" ]; then
                if [ "{{MISMATCH}}" = "warn" ]; then
                    echo "warn: core checkout ${head:0:9} differs from bridge/CORE_REF $expected;" >&2
                    echo "      building against the checkout. Pin before packaging: just core-pin" >&2
                else
                    echo "core checkout ${head:0:9} differs from bridge/CORE_REF $expected;" >&2
                    echo "check out $expected in $CORE (or pin core with 'just core-pin') before building." >&2
                    exit 1
                fi
            fi
        fi
    else
        echo "Cloning systemprompt-core beside this repo (the client needs it)."
        git clone --quiet https://github.com/systempromptio/systemprompt-core "$CORE"
        git -C "$CORE" checkout --quiet --detach "$(tr -d '\r\n' < "{{justfile_directory()}}/bridge/CORE_REF")"
    fi

# Serve the bridge GUI's web tree over HTTP so a browser can render it.
#
# The desktop webview is Windows/macOS only and reads its assets over a wry
# custom protocol, so this is the only way to see the GUI on Linux. Assets are
# served from disk: edit CSS/JS/HTML and refresh, no rebuild. Fixtures live in
# ../systemprompt-core/bin/bridge/web/dev/fixtures — pick one with
# ?fixture=<name>. Serves THIS repo's branded overlay.
bridge-preview PORT="4310": (core-checkout "warn")
    #!/usr/bin/env bash
    set -euo pipefail
    CORE="{{justfile_directory()}}/../systemprompt-core/bin/bridge"
    SYSTEMPROMPT_BRIDGE_WEB_OVERLAY="{{justfile_directory()}}/bridge/web" \
        cargo build --manifest-path "$CORE/Cargo.toml" --features dev-preview \
        --bin systemprompt-bridge
    echo "==> http://127.0.0.1:{{PORT}}/   (ctrl-c to stop)"
    "$CORE/target/debug/systemprompt-bridge" dev-web --port {{PORT}} --web-root "$CORE/web"

# Screenshot every bridge GUI state and build a contact sheet to review them.
# Starts its own preview, so it does not need `just bridge-preview` running.
bridge-shots PORT="4311":
    #!/usr/bin/env bash
    set -euo pipefail
    CORE="{{justfile_directory()}}/../systemprompt-core/bin/bridge"
    SYSTEMPROMPT_BRIDGE_WEB_OVERLAY="{{justfile_directory()}}/bridge/web" \
        cargo build --manifest-path "$CORE/Cargo.toml" --features dev-preview \
        --bin systemprompt-bridge
    if [ ! -d playwright/node_modules ]; then
        echo "==> installing Playwright dependencies"
        just e2e-install
    fi
    "$CORE/target/debug/systemprompt-bridge" dev-web --port {{PORT}} --web-root "$CORE/web" &
    PREVIEW_PID=$!
    # Stop the preview whichever way this exits, including a failing spec.
    trap 'kill $PREVIEW_PID 2>/dev/null || true' EXIT
    for _ in $(seq 1 40); do
        curl -sf "http://127.0.0.1:{{PORT}}/dev/fixtures" >/dev/null && break
        sleep 0.25
    done
    echo "==> rasterizing (Playwright)"
    cd playwright && BRIDGE_PREVIEW_URL="http://127.0.0.1:{{PORT}}" \
        npx playwright test tests/bridge-agents.spec.ts && cd ..
    echo "==> contact sheet"
    node scripts/bridge-contact-sheet.mjs
    echo "open playwright/bridge-shots/index.html"

# Package the branded bridge as a Linux release tarball into dist/
# Coordinated: bridge/ and the core sibling are both in the fingerprint, so a
# failed deploy retried on the same tree skips straight past this step.
bridge-package-linux:
    @scripts/build-coordinator.sh run bridge-package "" -- scripts/package-bridge-linux.sh

# Cross-compile the Windows bridge exe (x86_64-pc-windows-msvc via cargo-xwin —
# msvc is required: it statically links WebView2Loader, a -gnu build ships a
# bare exe that dies at start on "WebView2Loader.dll was not found") and stage
# it into dist/. Real releases come from .github/workflows/release.yml.
bridge-package-windows: core-checkout
    @scripts/build-coordinator.sh run bridge-package-windows "" -- scripts/package-bridge-windows.sh

# Installs the client if it is not there yet; re-running it with a fresh code
# re-binds the machine to whoever that code belongs to.
# Point Claude Code on THIS host at the gateway (CODE comes from /admin/profile)
connect CODE GATEWAY="http://localhost:8080":
    #!/usr/bin/env bash
    set -euo pipefail
    BASE="${DOWNLOAD_BASE:-https://github.com/systempromptio/systemprompt-internal/releases/download/bridge-v$(sed -n 's/^version = "\([0-9.]*\)"/\1/p' bridge/Cargo.toml | head -1)}"
    curl -fsSL "$BASE/install.sh" | sh -s -- \
        --download-base "$BASE" --gateway {{GATEWAY}} --code {{CODE}}

# Signing in is needed the first time only: the credential persists in a
# per-gateway volume, so later runs are just `just claude`. With no CODE the
# container walks you through the gateway's device-link page (sign in with
# Odoo, approve, paste the code); passing a CODE from /admin/profile skips
# that. `just claude-reset` signs out and makes signing in necessary again.
# Defaults to the production gateway; pass a second argument for a dev server
# (`just claude '' http://localhost:8080`).
# Claude Code, connected, in a container
claude CODE="" GATEWAY="https://internal.systemprompt.io":
    #!/usr/bin/env bash
    set -euo pipefail

    # The page prints the gateway as a browser sees it. Inside a container
    # localhost is the container, so rewrite it to the host alias. Keep the
    # original too: reaching it from the host is what separates "the gateway is
    # down" from "the gateway is up but the container cannot route to it".
    HOST_GATEWAY="{{GATEWAY}}"
    GATEWAY="{{GATEWAY}}"
    GATEWAY="${GATEWAY//localhost/host.docker.internal}"
    GATEWAY="${GATEWAY//127.0.0.1/host.docker.internal}"

    # Scope the session to this clone AND its gateway, the same way the Docker
    # Postgres project name is scoped, so several checkouts coexist.
    #
    # Two things force it. A credential is only valid for the gateway that
    # issued it, so sharing one home makes a second gateway look like a broken
    # sign-in: the PAT is found, whoami fails against the wrong host, and
    # bootstrap drops to asking for a code. And a fixed container name would
    # make a second clone attach to the first clone's session instead of
    # starting its own.
    REPO_SLUG="$(basename "{{justfile_directory()}}" | sed 's|[^A-Za-z0-9]|-|g')"
    REPO_HASH="$(printf '%s' "{{justfile_directory()}}" | sha256sum | cut -c1-8)"
    GW_SLUG="$(printf '%s' "$GATEWAY" | sed -e 's|^https\?://||' -e 's|[^A-Za-z0-9]|-|g')"
    SCOPE="${REPO_SLUG}-${REPO_HASH}-${GW_SLUG}"
    VOL="systemprompt-claude-${SCOPE}"
    NAME="systemprompt-claude-${SCOPE}"

    if [ "$(docker inspect -f '{{{{.State.Running}}}}' "$NAME" 2>/dev/null)" = "true" ]; then
        echo "Already running for this repo — opening another session in it."
        exec docker exec -it "$NAME" bash -lc claude
    fi
    docker rm -f "$NAME" >/dev/null 2>&1 || true

    if ! docker image inspect systemprompt-clean-client:local >/dev/null 2>&1; then
        echo "Image missing — building it first." >&2
        just clean-client-build
    fi

    # The client is a separate workspace and depends on systemprompt-bridge by
    # relative path into a sibling core checkout, so building it needs that
    # checkout present. `bridge-build` fetches it. Do this before the code is
    # spent: a first-run compile can outlive the code's 10-minute TTL, so
    # `just bridge-build` belongs in setup rather than here.
    BRIDGE="{{justfile_directory()}}/bridge/target/release/systemprompt-internal-bridge"
    if [ ! -f "$BRIDGE" ] || [ ! -x "$BRIDGE" ]; then
        echo "Client not built yet — fetching core and building it."
        echo "warn: a first build takes minutes and the code expires in 10." >&2
        echo "      If it is rejected as expired, issue a fresh one and re-run." >&2
        just bridge-build
    fi

    PORTS=()
    if ss -ltn 2>/dev/null | grep -q ':8767 '; then
        echo "warn: host port 8767 already in use — not publishing it." >&2
    else
        PORTS+=(-p 127.0.0.1:8767:8767)
    fi

    # Prove the container can reach the gateway BEFORE the code is spent —
    # otherwise an unroutable gateway surfaces as a connect error with the code
    # already consumed. --entrypoint is required: the image's entrypoint would
    # otherwise swallow this as its own arguments, print its banner, and exit 0,
    # so the check would pass without making a request.
    gateway_reachable() {
        docker run --rm --entrypoint curl \
            --add-host host.docker.internal:host-gateway \
            systemprompt-clean-client:local \
            -sf --max-time 5 "$GATEWAY/health" >/dev/null 2>&1
    }

    if ! gateway_reachable; then
        # Separate the two causes before touching anything. If the gateway does
        # not answer on the host either, it is down or on another port, and
        # rebinding it would be treating the wrong illness — Docker Desktop and
        # WSL2 proxy host.docker.internal to host loopback, so a 127.0.0.1 bind
        # is genuinely reachable there and the bind is often not the problem.
        if ! curl -sf --max-time 5 "$HOST_GATEWAY/health" >/dev/null 2>&1; then
            echo "ERROR: the gateway is not answering at $HOST_GATEWAY" >&2
            echo "" >&2
            echo "  It is not reachable from this host either, so it is down or" >&2
            echo "  listening on another port — not a container routing problem." >&2
            echo "  Start it, then re-run:" >&2
            echo "" >&2
            echo "      just start" >&2
            echo "      just server-status" >&2
            echo "" >&2
            echo "  Your code has not been used." >&2
            exit 1
        fi

        # The gateway is up on the host but the container cannot route to it.
        # On native Linux Docker that is exactly what a loopback bind causes,
        # and widening it is the documented remedy. Any other bind is left
        # alone rather than guessed at.
        if [ -x target/release/systemprompt ]; then
            SP="{{ justfile_directory() }}/target/release/systemprompt"
        else
            SP="{{ justfile_directory() }}/target/debug/systemprompt"
        fi
        # `show` prints its labels on stderr and its values on stdout, so the
        # human-readable form cannot be parsed by name. The JSON artifact can.
        # A parse failure yields an empty host, which matches nothing below and
        # so declines to remediate rather than guessing.
        BOUND_HOST="$("$SP" --json admin config server show --profile local 2>/dev/null \
            | python3 -c "import json,sys; d=json.load(sys.stdin); print(next((s.get('content','') for s in d.get('sections',[]) if s.get('heading')=='host'),''))" \
            2>/dev/null || true)"

        if [ "$BOUND_HOST" = "127.0.0.1" ] || [ "$BOUND_HOST" = "localhost" ]; then
            echo "notice: the gateway is bound to $BOUND_HOST, which no container can" >&2
            echo "        route to. Rebinding it to 0.0.0.0 and restarting." >&2
            echo "        This widens the gateway to your LAN — revert with:" >&2
            echo "            systemprompt admin config server set --host 127.0.0.1" >&2
            # Two things about this restart. `restart` needs an explicit target
            # — a bare `restart` exits 1 with "Must specify target (api, agent,
            # mcp)" — and the bind only affects the API listener, so restarting
            # `api` leaves the MCP servers up. And it SERVES in the foreground
            # rather than returning, so it has to be detached or it would hang
            # here forever; readiness is established by polling below, not by
            # the command exiting.
            "$SP" admin config server set --host 0.0.0.0 --profile local >/dev/null
            nohup "$SP" infra services restart api --profile local \
                >/dev/null 2>&1 &

            READY=0
            for _ in $(seq 1 45); do
                if gateway_reachable; then READY=1; break; fi
                sleep 1
            done
            if [ "$READY" -eq 0 ]; then
                echo "warn: the API server did not come back within 45s." >&2
                echo "      Check it with: just server-status" >&2
            fi
        fi
    fi

    if ! gateway_reachable; then
        echo "ERROR: the container cannot reach the gateway at $GATEWAY" >&2
        echo "" >&2
        echo "  The gateway answers on this host, so it is running — but the" >&2
        echo "  container cannot route to it, and it is bound to" >&2
        echo "  ${BOUND_HOST:-an unspecified host}, which rebinding did not fix." >&2
        echo "  Something between the two is in the way: a firewall on the" >&2
        echo "  docker bridge, or a docker network without host-gateway." >&2
        echo "" >&2
        echo "      just server-status" >&2
        echo "      curl -sf $HOST_GATEWAY/health      # from this host" >&2
        echo "" >&2
        echo "  Your code has not been used. Re-run this once the container can" >&2
        echo "  reach the gateway." >&2
        exit 1
    fi

    # No code on a repeat run: the stored credential is reused, and bootstrap
    # only falls back to asking when there is neither. Passing an empty value
    # would look like "a code was supplied" to that check.
    # With no code and no stored credential, bootstrap walks the device-link
    # sign-in interactively (open the gateway page, sign in with Odoo, approve,
    # paste the code) — which needs a terminal. Only a non-interactive caller
    # is refused, because for it that prompt is an indistinguishable hang.
    # Look for the credential itself, not merely the volume: a volume left over
    # from a run that never completed sign-in holds none.
    CODE_ENV=()
    if [ -n "{{CODE}}" ]; then
        CODE_ENV=(-e SYSTEMPROMPT_BRIDGE_CODE="{{CODE}}")
    elif ! docker run --rm --entrypoint test \
            -v "$VOL":/home/tester \
            systemprompt-clean-client:local \
            -f /home/tester/.config/systemprompt-internal/systemprompt-internal-bridge.pat >/dev/null 2>&1; then
        if [ ! -t 0 ]; then
            echo "ERROR: not signed in yet, and this is not an interactive terminal." >&2
            echo "" >&2
            echo "  Run from a terminal to sign in through the device-link page," >&2
            echo "  or pass a code from /admin/profile:" >&2
            echo "" >&2
            echo "      just claude <code>" >&2
            echo "" >&2
            echo "  Later runs need neither — the credential is kept." >&2
            exit 1
        fi
        echo "Not signed in yet — the container will walk you through the" >&2
        echo "device-link sign-in (browser sign-in, then paste the code)." >&2
    fi

    exec docker run -it --rm \
        --name "$NAME" \
        --hostname "${REPO_SLUG:0:24}" \
        --add-host host.docker.internal:host-gateway \
        -e SYSTEMPROMPT_BRIDGE_GATEWAY_URL="$GATEWAY" \
        "${CODE_ENV[@]}" \
        -e CLEAN_CLIENT_EXEC_CLAUDE=1 \
        -e CLEAN_CLIENT_ALLOW_STATE=1 \
        -v "$VOL":/home/tester \
        -v "$BRIDGE:/usr/local/bin/systemprompt-internal-bridge:ro" \
        -v "{{justfile_directory()}}/deploy/clean-client/bootstrap.sh:/usr/local/bin/bootstrap.sh:ro" \
        "${PORTS[@]}" \
        systemprompt-clean-client:local /usr/local/bin/bootstrap.sh

# ──────────────────────────────────────────────────────────────────────────────
# CLEAN CLIENT — a config-free Linux box for testing the Claude Code + bridge
# integration. See deploy/clean-client/README.md.
# ──────────────────────────────────────────────────────────────────────────────

# Build the clean-client image (context is deploy/clean-client only — no repo state)
clean-client-build *ARGS:
    docker build {{ARGS}} -f deploy/clean-client/Dockerfile -t systemprompt-clean-client:local deploy/clean-client

# Shell into a clean client. PERSIST=1 keeps the login across runs.
# GATEWAY overrides the gateway URL (default: this WSL host on :8080).
clean-client PERSIST="0" GATEWAY="http://host.docker.internal:8080":
    #!/usr/bin/env bash
    set -euo pipefail
    if ! docker image inspect systemprompt-clean-client:local >/dev/null 2>&1; then
        echo "Image missing — building it first." >&2
        just clean-client-build
    fi

    # Mount the bridge you just built, read-only. Absent is not fatal: you can
    # still test Claude Code against the gateway without the bridge installed.
    BRIDGE="{{justfile_directory()}}/bridge/target/release/systemprompt-internal-bridge"
    MOUNTS=()
    # -f as well as -x: a bare `docker run -v` against a missing path makes
    # docker create a root-owned DIRECTORY there, and `[ -x dir ]` is true, so
    # an -x-only test would happily mount the directory and report a bridge that
    # is not there.
    if [ -d "$BRIDGE" ]; then
        echo "ERROR: $BRIDGE is a directory (docker created it from a stale -v mount)." >&2
        echo "       Remove it with: sudo rmdir '$BRIDGE'" >&2
        exit 1
    fi
    if [ -f "$BRIDGE" ] && [ -x "$BRIDGE" ]; then
        MOUNTS+=(-v "$BRIDGE:/usr/local/bin/systemprompt-internal-bridge:ro")
    else
        echo "warn: $BRIDGE not found — run 'cd bridge && cargo build --release' for the full flow." >&2
    fi

    # PERSIST keeps ~/.config/systemprompt-internal and ~/.claude in a named volume so a PAT
    # survives a restart. Off by default: a throwaway HOME is the point.
    if [ "{{PERSIST}}" = "1" ]; then
        MOUNTS+=(-v systemprompt-clean-home:/home/tester -e CLEAN_CLIENT_ALLOW_STATE=1)
        echo "State persists in volume 'systemprompt-clean-home' — 'just clean-client-reset' wipes it."
    fi

    # 8767 is the bridge's plugin-OAuth loopback port; it must be reachable from
    # a Windows browser. It is NOT published if your primary distro already
    # holds it, since that bind would just fail.
    PORTS=()
    if ss -ltn 2>/dev/null | grep -q ':8767 '; then
        echo "warn: host port 8767 already in use — not publishing it. Plugin OAuth loopback will not work." >&2
    else
        PORTS+=(-p 127.0.0.1:8767:8767)
    fi

    # Note what is deliberately NOT here: no --env-file, no $HOME mounts, no
    # repo mount. The container must start from nothing.
    exec docker run -it --rm \
        --name systemprompt-clean-client \
        --hostname clean-client \
        --add-host host.docker.internal:host-gateway \
        -e SYSTEMPROMPT_BRIDGE_GATEWAY_URL="{{GATEWAY}}" \
        "${MOUNTS[@]}" "${PORTS[@]}" \
        systemprompt-clean-client:local

alias cc := clean-client-ready

# Clean client, signed in and ready: paste one code, then type `claude`.
clean-client-ready GATEWAY="http://host.docker.internal:8080":
    #!/usr/bin/env bash
    set -euo pipefail

    # Already running (another terminal has it): open a second shell in it
    # rather than failing on the name conflict.
    if [ "$(docker inspect -f '{{{{.State.Running}}}}' systemprompt-clean-client 2>/dev/null)" = "true" ]; then
        echo "Container 'systemprompt-clean-client' is already running — opening a shell in it."
        exec docker exec -it systemprompt-clean-client bash -l
    fi

    # Stopped leftover (a crashed run, or one started without --rm): restart it
    # and shell in, so anything written outside the /home/tester volume survives.
    # Only if it refuses to start do we reclaim the name.
    if docker inspect systemprompt-clean-client >/dev/null 2>&1; then
        echo "Container 'systemprompt-clean-client' is stopped — restarting it."
        if docker start systemprompt-clean-client >/dev/null 2>&1; then
            exec docker exec -it systemprompt-clean-client bash -l
        fi
        echo "warn: it would not restart — removing it and starting fresh." >&2
        docker rm -f systemprompt-clean-client >/dev/null
    fi

    if ! docker image inspect systemprompt-clean-client:local >/dev/null 2>&1; then
        echo "Image missing — building it first." >&2
        just clean-client-build
    fi

    BRIDGE="{{justfile_directory()}}/bridge/target/release/systemprompt-internal-bridge"
    if [ ! -f "$BRIDGE" ] || [ ! -x "$BRIDGE" ]; then
        echo "ERROR: $BRIDGE not found — run 'just bridge-build' first." >&2
        exit 1
    fi

    PORTS=()
    if ss -ltn 2>/dev/null | grep -q ':8767 '; then
        echo "warn: host port 8767 already in use — not publishing it." >&2
    else
        PORTS+=(-p 127.0.0.1:8767:8767)
    fi

    # State persists so a second run reuses the PAT instead of burning a code;
    # CLEAN_CLIENT_ALLOW_STATE tells the entrypoint that reuse is deliberate
    # here rather than host config leaking in.
    exec docker run -it --rm \
        --name systemprompt-clean-client \
        --hostname clean-client \
        --add-host host.docker.internal:host-gateway \
        -e SYSTEMPROMPT_BRIDGE_GATEWAY_URL="{{GATEWAY}}" \
        -e CLEAN_CLIENT_ALLOW_STATE=1 \
        -v systemprompt-clean-home:/home/tester \
        -v "$BRIDGE:/usr/local/bin/systemprompt-internal-bridge:ro" \
        -v "{{justfile_directory()}}/deploy/clean-client/bootstrap.sh:/usr/local/bin/bootstrap.sh:ro" \
        "${PORTS[@]}" \
        systemprompt-clean-client:local /usr/local/bin/bootstrap.sh

# End-to-end: run the published installer with a PAT and assert managed MCP (see script header for how to mint the PAT)
clean-client-install PAT GATEWAY="http://host.docker.internal:8080":
    GATEWAY="{{GATEWAY}}" scripts/clean-client-install.sh "{{PAT}}"

# Drops this clone's sessions and the credentials they stored, so the next run
# redeems a fresh code. ALL=1 signs out every clone on this host.
# Sign out of `just claude` (this repo; ALL=1 for every repo)
claude-reset ALL="0":
    #!/usr/bin/env bash
    set -euo pipefail
    # Scoped to this clone by default: signing every other checkout out because
    # one of them wanted a clean slate is not what anyone means by "reset".
    if [ "{{ALL}}" = "1" ]; then
        FILTER='name=^systemprompt-claude-'
        echo "Signing out every repo on this host."
    else
        REPO_SLUG="$(basename "{{justfile_directory()}}" | sed 's|[^A-Za-z0-9]|-|g')"
        REPO_HASH="$(printf '%s' "{{justfile_directory()}}" | sha256sum | cut -c1-8)"
        FILTER="name=^systemprompt-claude-${REPO_SLUG}-${REPO_HASH}-"
    fi
    docker ps -aq --filter "$FILTER" | while read -r c; do
        docker rm -f "$c" >/dev/null 2>&1 && echo "removed container $c"
    done
    FOUND=0
    for v in $(docker volume ls -q --filter "$FILTER"); do
        docker volume rm "$v" >/dev/null 2>&1 && { echo "removed $v"; FOUND=1; }
    done
    [ "$FOUND" = "1" ] || echo "Nothing to sign out of."
    echo "'just claude <code>' will start from nothing."

# Wipe the persisted clean-client state volume
clean-client-reset:
    -docker rm -f systemprompt-clean-client 2>/dev/null
    -docker volume rm systemprompt-clean-home
    @echo "Clean-client state wiped."

# Isolated dev sandbox on a real project: clean client + the repo mounted at
# /workspace/project. HOME stays virgin (device-link auth as usual); only the
# project directory crosses into the container. The image ships Playwright +
# Chromium so the dev_test skill works against the mounted project.
dev-sandbox REPO PERSIST="0" GATEWAY="http://host.docker.internal:8080":
    #!/usr/bin/env bash
    set -euo pipefail
    REPO_ABS="$(readlink -f "{{REPO}}")"
    if [ ! -d "$REPO_ABS" ]; then
        echo "ERROR: {{REPO}} is not a directory" >&2
        exit 1
    fi
    if ! docker image inspect systemprompt-clean-client:local >/dev/null 2>&1; then
        echo "Image missing — building it first." >&2
        just clean-client-build
    fi
    BRIDGE="{{justfile_directory()}}/bridge/target/release/systemprompt-internal-bridge"
    MOUNTS=(-v "$REPO_ABS:/workspace/project")
    if [ -d "$BRIDGE" ]; then
        echo "ERROR: $BRIDGE is a directory (docker created it from a stale -v mount)." >&2
        echo "       Remove it with: sudo rmdir '$BRIDGE'" >&2
        exit 1
    fi
    if [ -f "$BRIDGE" ] && [ -x "$BRIDGE" ]; then
        MOUNTS+=(-v "$BRIDGE:/usr/local/bin/systemprompt-internal-bridge:ro")
    else
        echo "warn: $BRIDGE not found — run 'cd bridge && cargo build --release' for the full flow." >&2
    fi
    if [ "{{PERSIST}}" = "1" ]; then
        MOUNTS+=(-v systemprompt-clean-home:/home/tester -e CLEAN_CLIENT_ALLOW_STATE=1)
        echo "State persists in volume 'systemprompt-clean-home' — 'just clean-client-reset' wipes it."
    fi
    PORTS=()
    if ss -ltn 2>/dev/null | grep -q ':8767 '; then
        echo "warn: host port 8767 already in use — not publishing it. Plugin OAuth loopback will not work." >&2
    else
        PORTS+=(-p 127.0.0.1:8767:8767)
    fi
    exec docker run -it --rm \
        --name systemprompt-dev-sandbox \
        --hostname dev-sandbox \
        --add-host host.docker.internal:host-gateway \
        -e SYSTEMPROMPT_BRIDGE_GATEWAY_URL="{{GATEWAY}}" \
        "${MOUNTS[@]}" "${PORTS[@]}" \
        systemprompt-clean-client:local

# Install the Playwright e2e suite's dependencies (playwright/ directory)
e2e-install:
    cd playwright && npm install && npx playwright install chromium

# Run the Playwright browser suite against a running gateway (GATEWAY_URL env
# overrides the default http://localhost:8080). Not part of `just validate` —
# it needs a live stack: `just start` first. (`just e2e` is the Rust
# end-to-end suite; this drives the browser.)
playwright *ARGS:
    cd playwright && npx playwright test {{ARGS}}

# Render every artifact type through the real MCP UI renderer, rasterize each in
# light / dark / 375px-narrow, and assemble a contact sheet.
#
# Local and opt-in: CI has no browser, and cross-machine font rendering makes
# golden-image diffing flaky. The functional half — that all twelve types render
# and carry the brand theme — is a normal Rust test that CI does run.
artifact-gallery:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "==> rendering artifacts (Rust)"
    # The wire entry (`wire-crm-lead-search.html`) comes from `just e2e`, which
    # needs a database; this render pass does not, so it stays runnable on its
    # own and the spec picks the wire entry up whenever it is present.
    cargo nextest run --manifest-path tests/Cargo.toml -p e2e-tests -E 'test(artifact_gallery)' --no-capture
    if [ ! -d playwright/node_modules ]; then
        echo "==> installing Playwright dependencies"
        just e2e-install
    fi
    echo "==> rasterizing (Playwright)"
    cd playwright && npx playwright test tests/artifact-gallery.spec.ts && cd ..
    # Drives core's real artifact shell as a host would. It reads the HTML the
    # step above just rendered, because `frame.js` is include_str!'d into each
    # artifact at render time — run against a stale gallery it tests the old
    # copy of the very file it is checking.
    echo "==> host theme handshake (Playwright)"
    cd playwright && npx playwright test tests/artifact-host-theme.spec.ts && cd ..
    echo "==> contact sheet"
    node scripts/artifact-contact-sheet.mjs
    echo "open playwright/artifact-shots/index.html"

# Test build without pushing
docker-test:
    just build-all
    just docker-build test
    @echo "Docker build successful! Image: systemprompt-template:test"

# ══════════════════════════════════════════════════════════════════════════════
# AIR-GAPPED SCENARIO
# ══════════════════════════════════════════════════════════════════════════════

# Bring up the network-isolated air-gap stack (postgres + mock-inference + app + monitor + ingress)
airgap-up:
    #!/usr/bin/env bash
    set -euo pipefail
    # Dockerfile.airgap-prebuilt COPYs the host-built binaries from
    # deploy/scenarios/airgap/.bin/ — `target` is a symlink to a shared cargo
    # cache that buildkit can't follow, so we dereference-copy them in first
    # (mirrors scaled-up).
    if [[ ! -x target/release/systemprompt || ! -x target/release/systemprompt-mcp-agent ]]; then
        echo "ERROR: release binaries missing. Run: just build --release" >&2
        exit 1
    fi
    mkdir -p deploy/scenarios/airgap/.bin
    cp -L target/release/systemprompt           deploy/scenarios/airgap/.bin/systemprompt
    cp -L target/release/systemprompt-mcp-agent deploy/scenarios/airgap/.bin/systemprompt-mcp-agent
    docker compose -f deploy/scenarios/airgap/docker-compose.airgap.yml up -d --build

# Tear down the air-gap stack and remove its volumes
airgap-down:
    docker compose -f deploy/scenarios/airgap/docker-compose.airgap.yml down -v

# ONE-COMMAND air-gap proof. Ensures the sealed stack is up (builds the image
# only if it is missing), warm-builds the loadtest crate so the run emits no
# compiler spew, runs all three assertion scripts (01 egress, 02 load,
# 03 governance) WITHOUT dying on the first failure, then prints a single
# PASS/FAIL summary. Leaves the stack up for inspection by default — pass
# TEARDOWN=true to remove it (and its volumes) at the end.
#
#   just airgap                # run, leave stack up
#   just airgap TEARDOWN=true  # run, then tear down
airgap TEARDOWN="false":
    #!/usr/bin/env bash
    set -uo pipefail
    COMPOSE_FILE="deploy/scenarios/airgap/docker-compose.airgap.yml"
    PORT="${AIRGAP_HTTP_PORT:-8090}"
    LOADTEST_MANIFEST="../systemprompt-core/crates/tests/loadtest/Cargo.toml"

    # 1. Ensure the stack is up. Build the image only if it is not present yet
    #    (a first-time build pulls in ../systemprompt-core and takes ~10 min).
    if curl -fsS -o /dev/null --max-time 3 "http://localhost:${PORT}/api/v1/health" 2>/dev/null; then
      echo "  air-gap stack already healthy on :${PORT}"
    else
      if docker compose -f "$COMPOSE_FILE" config --images 2>/dev/null \
         | xargs -r -I{} docker image inspect {} >/dev/null 2>&1; then
        echo "  air-gap image present — starting stack (no rebuild)"
        docker compose -f "$COMPOSE_FILE" up -d
      else
        echo "  air-gap image missing — building stack (first run, ~10 min)"
        docker compose -f "$COMPOSE_FILE" up -d --build
      fi
      echo "  waiting for app healthcheck on :${PORT} ..."
      for i in $(seq 1 120); do
        if curl -fsS -o /dev/null "http://localhost:${PORT}/api/v1/health" 2>/dev/null; then
          echo "  app healthy after ${i}s"
          break
        fi
        sleep 1
      done
    fi

    # 2. Warm-build the loadtest crate quietly so STEP 02's `cargo run` emits no
    #    build output mid-demo. Non-fatal: 02-load.sh re-checks the manifest.
    if [[ -f "$LOADTEST_MANIFEST" ]]; then
      echo "  warm-building the loadtest crate ..."
      cargo build --quiet --manifest-path "$LOADTEST_MANIFEST" 2>/dev/null || true
    else
      echo "  loadtest crate not found at ${LOADTEST_MANIFEST} — skipping warm-build" >&2
      echo "  (it is unpublished systemprompt-core dev tooling; 02-load.sh will build it on demand if present)" >&2
    fi

    # 3. Run all three scripts, capturing each exit code (do NOT stop on first
    #    failure — the operator must see the full picture).
    declare -A RESULT
    for s in 01-egress-assert 02-load 03-governance; do
      echo ""
      if "./demo/scenarios/airgap/${s}.sh"; then
        RESULT[$s]="PASS"
      else
        RESULT[$s]="FAIL"
      fi
    done

    # 4. Single PASS/FAIL summary.
    echo ""
    echo "══════════════════════════════════════════════════════════"
    echo "  AIR-GAP PROOF SUMMARY"
    echo "══════════════════════════════════════════════════════════"
    OVERALL=0
    for s in 01-egress-assert 02-load 03-governance; do
      printf "  %-22s %s\n" "$s" "${RESULT[$s]}"
      [[ "${RESULT[$s]}" == "PASS" ]] || OVERALL=1
    done
    echo "══════════════════════════════════════════════════════════"
    [[ "$OVERALL" -eq 0 ]] && echo "  RESULT: PASS" || echo "  RESULT: FAIL"

    # 5. Optional teardown.
    if [[ "{{TEARDOWN}}" == "true" ]]; then
      echo ""
      echo "  TEARDOWN=true — removing the air-gap stack and volumes"
      just airgap-down
    fi

    exit "$OVERALL"

# Run the air-gap demo scripts in sequence, stopping on first failure.
# Policies (quotas/safety) ship as services/gateway/policies.yaml and are
# ingested by the publish_pipeline job at server boot. Model exposure lives
# in the profile provider registry (profile.providers in
# .systemprompt/profiles/airgap/profile.yaml).
airgap-test:
    #!/usr/bin/env bash
    set -euo pipefail
    ./demo/scenarios/airgap/01-egress-assert.sh
    ./demo/scenarios/airgap/02-load.sh
    ./demo/scenarios/airgap/03-governance.sh

# Reproducibility proof: tear down (incl. volumes), bring back up reusing the
# already-built image, run the full assertion suite from zero state. Prints
# wall-clock time. Use this in front of a reviewer who wants to see the demo
# work from a clean container + clean database, without a 10-minute image
# rebuild. Image-level reproducibility is a separate concern — see
# demo/scenarios/airgap/architecture.md §9 (the [patch.crates-io] block
# requires systemprompt-core >= 0.10.4 to be published before the image can
# be rebuilt from this repo in isolation).
airgap-fresh-test:
    #!/usr/bin/env bash
    set -euo pipefail
    COMPOSE_FILE="deploy/scenarios/airgap/docker-compose.airgap.yml"
    # Refuse to run if the image isn't already built — the rebuild path needs
    # the sibling systemprompt-core repo and a 10-minute window, and silently
    # falling into that on a demo machine is a bad surprise.
    if ! docker image inspect airgap-app >/dev/null 2>&1 \
       && ! docker compose -f "$COMPOSE_FILE" config --images 2>/dev/null | head -1 | xargs -I{} docker image inspect {} >/dev/null 2>&1; then
      echo "ERROR: app image not present. First-time build needed:" >&2
      echo "  just airgap-up   # builds the image (~10 min, needs ../systemprompt-core)" >&2
      exit 1
    fi
    START=$(date +%s)
    just airgap-down
    # No --build: reuse the existing image. This is the from-zero DATA reset,
    # not the from-zero BUILD reset.
    docker compose -f "$COMPOSE_FILE" up -d
    echo "Waiting for app healthcheck..."
    for i in $(seq 1 120); do
      if curl -fsS -o /dev/null "http://localhost:${AIRGAP_HTTP_PORT:-8090}/api/v1/health" 2>/dev/null; then
        echo "App healthy after ${i}s"
        break
      fi
      sleep 1
    done
    just airgap-test
    END=$(date +%s)
    echo ""
    echo "═══════════════════════════════════════════════════════"
    echo "  FRESH AIR-GAP RUN COMPLETE in $((END - START))s"
    echo "═══════════════════════════════════════════════════════"

# ══════════════════════════════════════════════════════════════════════════════
# SCALED / DISTRIBUTED SCENARIO
# ══════════════════════════════════════════════════════════════════════════════

# Bring up the multi-replica scaled stack (postgres primary/replica + N app replicas + 1 scheduler + nginx LB)
scaled-up REPLICAS="3":
    #!/usr/bin/env bash
    set -euo pipefail
    # Stage the host-built binaries into a real dir inside the build context —
    # `target` is a symlink to a shared cargo cache that buildkit can't follow.
    if [[ ! -x target/release/systemprompt || ! -x target/release/systemprompt-mcp-agent ]]; then
        echo "ERROR: release binaries missing. Run: just build --release" >&2
        exit 1
    fi
    mkdir -p deploy/scenarios/scaled/.bin
    cp -L target/release/systemprompt           deploy/scenarios/scaled/.bin/systemprompt
    cp -L target/release/systemprompt-mcp-agent deploy/scenarios/scaled/.bin/systemprompt-mcp-agent
    docker compose -f deploy/scenarios/scaled/docker-compose.scaled.yml up -d --build --scale app={{REPLICAS}}

# Tear down the scaled stack and remove its volumes
scaled-down:
    docker compose -f deploy/scenarios/scaled/docker-compose.scaled.yml down -v

# ONE COMMAND: reset → build → up → wait-for-health → mint token → run all fast
# proofs → capture logs → single verdict. Leaves the stack up by default.
#   just scaled-demo                # 3 replicas, stack left up
#   REPLICAS=5 just scaled-demo     # scale wider
#   KEEP=0 just scaled-demo         # tear down at the end
#   SOAK=1 just scaled-demo         # also run the ~1h soak (long!)
scaled-demo:
    #!/usr/bin/env bash
    set -uo pipefail
    chmod +x demo/scenarios/scaled/run.sh
    ./demo/scenarios/scaled/run.sh

# Run the scaled demo scripts in sequence against an ALREADY-RUNNING stack.
# Prefer `just scaled-demo` (full lifecycle). Use this only when the stack is
# already up and healthy. Skips 02-soak.sh — the long (~1h) sustained soak; run
# it on its own when needed: ./demo/scenarios/scaled/02-soak.sh
scaled-test:
    #!/usr/bin/env bash
    set -euo pipefail
    chmod +x demo/scenarios/scaled/01-load.sh \
             demo/scenarios/scaled/03-replica-distribution.sh \
             demo/scenarios/scaled/04-scheduler-isolation.sh
    ./demo/scenarios/scaled/01-load.sh
    ./demo/scenarios/scaled/03-replica-distribution.sh
    ./demo/scenarios/scaled/04-scheduler-isolation.sh

# ══════════════════════════════════════════════════════════════════════════════
# ADMIN & PLUGINS
# ══════════════════════════════════════════════════════════════════════════════

# Generate WebAuthn setup token for admin user
webauthn-admin EMAIL:
    {{CLI}} admin users webauthn generate-setup-token --email "{{EMAIL}}"

# Update Anthropic official plugins from vendor submodule and reimport
update-anthropic-plugins:
    git submodule update --remote vendor/knowledge-work-plugins
    {{CLI}} infra jobs run import_anthropic_plugins

# ══════════════════════════════════════════════════════════════════════════════
# TERMINAL RECORDINGS (README SVGs)
# ══════════════════════════════════════════════════════════════════════════════

# Regenerate terminal SVG recordings. Pass numbers to limit scope, e.g. `just record-svgs 3 7`.
record-svgs *NUMBERS:
    ./demo/recording/svg/record.sh {{NUMBERS}}

# ══════════════════════════════════════════════════════════════════════════════
# BENCHMARKS
# ══════════════════════════════════════════════════════════════════════════════

# Benchmark governance endpoint. Downloads `hey` for the host OS/arch on first run.
benchmark REQUESTS="200" CONCURRENCY="100":
    #!/usr/bin/env bash
    set -e
    # Use system hey if available, else /tmp/hey
    if command -v hey >/dev/null 2>&1; then
        HEY="$(command -v hey)"
    else
        HEY="/tmp/hey"
    fi
    # Re-download if the cached binary can't execute here (e.g. Linux hey on a Mac).
    if ! { [[ -x "$HEY" ]] && "$HEY" --help >/dev/null 2>&1; }; then
        rm -f "$HEY"
        HEY="/tmp/hey"
        OS_ARCH="$(uname -s)/$(uname -m)"
        case "$OS_ARCH" in
            Darwin/*)
                HEY_URL="https://hey-release.s3.us-east-2.amazonaws.com/hey_darwin_amd64"
                echo "Installing hey from $HEY_URL..."
                curl -fsSL "$HEY_URL" -o "$HEY" && chmod +x "$HEY"
                ;;
            Linux/x86_64|Linux/amd64)
                HEY_URL="https://hey-release.s3.us-east-2.amazonaws.com/hey_linux_amd64"
                echo "Installing hey from $HEY_URL..."
                if ! curl -fsSL "$HEY_URL" -o "$HEY"; then
                    echo "ERROR: failed to download hey. Run: sudo apt-get install hey" >&2; exit 1
                fi
                chmod +x "$HEY"
                ;;
            *) echo "ERROR: no prebuilt hey for $OS_ARCH. Install with 'brew install hey' or 'go install github.com/rakyll/hey@latest'." >&2; exit 1 ;;
        esac
        if ! "$HEY" --help >/dev/null 2>&1; then
            echo "ERROR: hey won't run on $OS_ARCH." >&2
            if [[ "$OS_ARCH" == "Darwin/arm64" ]]; then
                echo "       Apple Silicon: 'softwareupdate --install-rosetta' or 'brew install hey'." >&2
            else
                echo "       Install manually: 'sudo apt-get install hey' or 'go install github.com/rakyll/hey@latest'." >&2
            fi
            rm -f "$HEY"; exit 1
        fi
    fi
    TOKEN_FILE="demo/.token"
    if [[ ! -f "$TOKEN_FILE" ]]; then
        echo "ERROR: No token. Run: ./demo/00-preflight.sh" >&2
        exit 1
    fi
    TOKEN=$(cat "$TOKEN_FILE")
    echo "Governance endpoint: {{REQUESTS}} requests, {{CONCURRENCY}} concurrent"
    echo ""
    "$HEY" -n {{REQUESTS}} -c {{CONCURRENCY}} -m POST \
        -H "Authorization: Bearer $TOKEN" \
        -H "Content-Type: application/json" \
        -d '{"hook_event_name":"PreToolUse","tool_name":"Read","agent_id":"developer_agent","session_id":"bench","tool_input":{"file_path":"/src/main.rs"}}' \
        "http://localhost:8080/api/public/hooks/govern?plugin_id=enterprise-demo"

# Syntax-check install.sh (install.sh is the user-facing installer)
install-sh-test:
    bash -n scripts/install.sh
    shellcheck scripts/install.sh 2>/dev/null || echo "(install shellcheck to lint: apt install shellcheck)"

# Check the Nix flake builds + runs
flake-check:
    nix flake check
    nix run .# -- --version

# --- Release ------------------------------------------------------------

# Adopt a published core (patch dormant): bump every version pin to it —
# lockstep, so the workspace, bridge, image and every core pin move together
# and bridge/CORE_REF becomes its tag — refresh all three lockfiles, and gate
# locally (migrate + build + clippy). Then record the schema rung
# (`just schema-baseline`), `just verify`, push `next`, and `just release`.
# See docs/RELEASING.md.
#
# LOCAL-ONLY. The migrate step names `--profile local` explicitly: the 0.51.0
# bump ran a bare `infra db migrate` after `deploy-check` had switched the CLI
# session to `production`, and the migration targeted the live database. The
# explicit profile makes the target independent of session state, and the
# failure is no longer swallowed — a migrate that cannot run stops the bump.
# All three lockfiles are re-resolved; `cargo update -w` covers the root only
# (scripts/check-core-crate-versions.sh fails if they disagree).
core-bump version:
    @! grep -q '^\[patch\.crates-io\]' Cargo.toml || (echo "ERROR: [patch.crates-io] is active — publish core and make the patch dormant first" && exit 1)
    scripts/sync-release-version.sh {{version}}
    cargo update -w
    cargo update -w --manifest-path tests/Cargo.toml
    cargo update -w --manifest-path bridge/Cargo.toml
    just db-up
    cargo run --bin systemprompt -- infra db migrate --profile local
    just build
    just clippy
    @echo "core-bump {{version}} complete — next: just schema-baseline, just verify, commit, push next, then: just release {{version}}"

# Pin bridge/CORE_REF to a core `next` commit while [patch.crates-io] is
# active (the sibling checkout's HEAD by default). CI checks that ref out.
core-pin REF="":
    #!/usr/bin/env bash
    set -euo pipefail
    ref="{{REF}}"
    core="${CORE_REPO:-../systemprompt-core}"
    [ -n "$ref" ] || ref="$(git -C "$core" rev-parse HEAD)"
    # check-core-ref.sh accepts only a vX.Y.Z tag or a full 40-char SHA, so expand abbreviations.
    case "$ref" in v[0-9]*) ;; *) ref="$(git -C "$core" rev-parse --verify "${ref}^{commit}")" ;; esac
    printf '%s\n' "$ref" > bridge/CORE_REF
    echo "bridge/CORE_REF = $ref"

# Promote the exact green next-push candidate through a frozen PR onto main
# (scripts/release.sh). The first run opens the PR; repeat after the PR's own
# Gates run finishes and it merges. release.yml then builds, proves and
# publishes v<version> and bridge-v<version> from the merge commit.
release version:
    bash scripts/release.sh {{version}}

# Install the compiled server + MCP binaries from a GitHub Release instead of
# building them: `systemprompt-internal-<version>-<os>-<arch>.tar.gz` holds
# bin/systemprompt and every bin/systemprompt-mcp-*, built by the same
# `cargo build --release --workspace` as the image. They land in
# target/release/, where `just start` and the MCP validator already look — so
# a clone never needs a Rust toolchain. Linux amd64/arm64 and macOS arm64;
# verified against the release's SHA256SUMS. Default version = the workspace
# version in Cargo.toml.
fetch-release VERSION="":
    #!/usr/bin/env bash
    set -euo pipefail
    v="{{VERSION}}"
    [ -n "$v" ] || v=$(sed -n 's/^version = "\([0-9.]*\)"/\1/p' Cargo.toml | head -1)
    case "$(uname -s)-$(uname -m)" in
        Linux-x86_64) plat=linux-amd64 ;;
        Linux-aarch64|Linux-arm64) plat=linux-arm64 ;;
        Darwin-arm64) plat=darwin-arm64 ;;
        *) echo "fetch-release: no release tarball for $(uname -s) $(uname -m); use the image or 'just build --release'." >&2; exit 1 ;;
    esac
    name="systemprompt-internal-$v-$plat.tar.gz"
    tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
    echo "==> downloading $name from release v$v"
    gh release download "v$v" -R systempromptio/systemprompt-internal -p "$name" -p SHA256SUMS -D "$tmp" \
        || { echo "fetch-release: release v$v has no $name (gh auth, or the release does not exist yet)." >&2; exit 1; }
    want="$(grep " $name\$" "$tmp/SHA256SUMS" | cut -d' ' -f1)"
    got="$( (sha256sum "$tmp/$name" 2>/dev/null || shasum -a 256 "$tmp/$name") | cut -d' ' -f1)"
    [ -n "$want" ] && [ "$got" = "$want" ] || { echo "fetch-release: $name does not match the release SHA256SUMS" >&2; exit 1; }
    tar xzf "$tmp/$name" -C "$tmp"
    mkdir -p target/release
    install -m 0755 "$tmp/${name%.tar.gz}"/bin/* target/release/
    echo "==> installed into target/release/:"; ls -1 target/release/systemprompt target/release/systemprompt-mcp-* | sed 's/^/    /'
    target/release/systemprompt --version

# Deploy a published release to production from a clean worktree of
# origin/main — never from this tree, which may hold a peer's work or an
# active [patch.crates-io]. Binaries come from the vVERSION release (verified
# against its SHA256SUMS), web/dist is rendered inside the worktree, the Fly
# image digest must move, and /health is watched for five minutes. Leaves the
# worktree in place on failure for inspection. Linux x86_64 hosts only.
deploy-release VERSION:
    bash scripts/deploy-release.sh {{VERSION}}

# --- Odoo companion app (Fly) --------------------------------------------
# Odoo CE runs as its own Fly app (sp-88906bfd0afd-odoo) with a volume for
# the filestore and a dedicated DB on systemprompt-db-prod. Tenant deploys
# never touch it. See deploy/fly/odoo/.

ODOO_APP := "sp-88906bfd0afd-odoo"
ODOO_DB_NAME := "odoo_88906bfd0afd"

# Deploy the Odoo companion app (single machine → brief downtime, immediate strategy)
odoo-deploy:
    flyctl deploy deploy/fly/odoo --strategy immediate

odoo-status:
    flyctl status -a {{ODOO_APP}}

odoo-logs *FLAGS:
    flyctl logs -a {{ODOO_APP}} {{FLAGS}}

# Set the two Odoo secrets: just odoo-secrets <db_password> <admin_passwd>
odoo-secrets DB_PASSWORD ADMIN_PASSWD:
    flyctl secrets set -a {{ODOO_APP}} ODOO_DB_PASSWORD='{{DB_PASSWORD}}' ODOO_ADMIN_PASSWD='{{ADMIN_PASSWD}}'

# Push the SMTP relay credentials to the Odoo app as Fly secrets. Reads them
# from the production profile (shared with systemprompt-web's Resend relay),
# so there is no separate key to rotate. Restarts the machine (single machine
# → brief downtime). Run once, then `just odoo-mail-config`.
odoo-mail-secrets:
    #!/usr/bin/env bash
    set -euo pipefail
    SECRETS=".systemprompt/profiles/production/secrets.json"
    [ -f "$SECRETS" ] || { echo "ERROR: $SECRETS not found" >&2; exit 1; }
    read_secret() {
        python3 -c 'import json,sys; v=json.load(open(sys.argv[1])).get(sys.argv[2],""); sys.exit("ERROR: missing "+sys.argv[2]+" in profile secrets") if not v else print(v)' "$SECRETS" "$1"
    }
    SMTP_HOST=$(read_secret smtp_host)
    SMTP_PORT=$(read_secret smtp_port)
    SMTP_USER=$(read_secret smtp_username)
    SMTP_PASS=$(read_secret smtp_password)
    flyctl secrets set -a {{ODOO_APP}} \
      SMTP_HOST="$SMTP_HOST" SMTP_PORT="$SMTP_PORT" \
      SMTP_USER="$SMTP_USER" SMTP_PASSWORD="$SMTP_PASS"

# Configure outgoing mail in the Odoo database (idempotent, and it verifies
# the relay before returning). Needs `just odoo-mail-secrets` first.
# Outbound only — see the note in deploy/fly/odoo/configure-mail.py.
odoo-mail-config:
    #!/usr/bin/env bash
    set -euo pipefail
    # base64 so the whole script crosses `ssh -C` as one argument-safe blob.
    SCRIPT=$(base64 -w0 < deploy/fly/odoo/configure-mail.py 2>/dev/null || base64 < deploy/fly/odoo/configure-mail.py | tr -d '\n')
    flyctl ssh console -a {{ODOO_APP}} -C "/bin/bash -lc \"echo $SCRIPT | base64 -d | odoo shell -c /tmp/odoo.conf -d {{ODOO_DB_NAME}} --no-http --log-level=warn\""

# The entrypoint already pre-generates these on every boot; this recipe is for
# repairing a live machine that is serving a /web/assets/... 500 (unstyled UI).
# Pre-generate the Odoo asset bundles on the running machine
odoo-assets:
    #!/usr/bin/env bash
    set -euo pipefail
    # base64 so the whole script crosses `ssh -C` as one argument-safe blob.
    SCRIPT=$(base64 -w0 < deploy/fly/odoo/pregenerate-assets.py 2>/dev/null || base64 < deploy/fly/odoo/pregenerate-assets.py | tr -d '\n')
    flyctl ssh console -a {{ODOO_APP}} -C "/bin/bash -lc \"echo $SCRIPT | base64 -d | odoo shell -c /tmp/odoo.conf -d {{ODOO_DB_NAME}} --no-http --log-level=warn\""

# One-time: create the odoo role + database on systemprompt-db-prod.
# Needs the cluster superuser password: just odoo-provision-db <role_pw>
# (connects via db.systemprompt.io:5432; prompts for the postgres password)
odoo-provision-db ROLE_PASSWORD:
    psql -h db.systemprompt.io -p 5432 -U postgres -d postgres \
      -v pw='{{ROLE_PASSWORD}}' -f deploy/fly/odoo/provision-db.sql

# Backup: volume snapshot (filestore) + pg_dump (database) → backups/odoo/
# pg_dump/pg_restore auth: export PGPASSWORD=<odoo role pw> (or use ~/.pgpass)
odoo-backup:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p backups/odoo
    vol=$(flyctl volumes list -a {{ODOO_APP}} --json | python3 -c "import json,sys;print(json.load(sys.stdin)[0]['id'])")
    flyctl volumes snapshots create "$vol" -a {{ODOO_APP}}
    out="backups/odoo/{{ODOO_DB_NAME}}_$(date +%Y%m%d_%H%M%S).dump"
    pg_dump -h db.systemprompt.io -p 5432 -U {{ODOO_DB_NAME}} -d {{ODOO_DB_NAME}} -Fc -f "$out"
    echo "wrote $out"

# Restore a pg_dump made by odoo-backup: just odoo-restore backups/odoo/<file>.dump
odoo-restore DUMP:
    pg_restore -h db.systemprompt.io -p 5432 -U {{ODOO_DB_NAME}} -d {{ODOO_DB_NAME}} --clean --if-exists --no-owner "{{DUMP}}"

# Local: create the odoo role + initialise the odoo_local DB on the local dev
# postgres (after `just db-up`). Idempotent — safe to re-run. Mirrors prod's
# first boot (deploy/fly/odoo/entrypoint.sh): `-i base --without-demo=all`,
# so no web DB-manager wizard is needed. Login afterwards: admin / admin.
odoo-local-init:
    #!/usr/bin/env bash
    set -euo pipefail
    COMPOSE_FILE=".systemprompt/docker/local.yaml"
    PROJECT="$(just _project_name local)"
    # Derive the host Postgres port from the compose file (setup-local wrote it).
    PG_PORT=$(sed -n 's/^ *- *"\([0-9]*\):5432"/\1/p' "$COMPOSE_FILE" | head -n1)
    if [ -z "$PG_PORT" ]; then
        echo "ERROR: could not read the Postgres host port from $COMPOSE_FILE" >&2
        exit 1
    fi
    PGPASSWORD=123 psql -h localhost -p "$PG_PORT" -U systemprompt -d systemprompt \
      -c "DO \$\$ BEGIN IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname='odoo') THEN CREATE USER odoo WITH PASSWORD 'odoo' CREATEDB; END IF; END \$\$;"
    EXISTS=$(PGPASSWORD=123 psql -h localhost -p "$PG_PORT" -U systemprompt -d systemprompt \
      -tAc "SELECT 1 FROM pg_database WHERE datname='odoo_local'")
    if [ "$EXISTS" = "1" ]; then
        echo "odoo_local database already exists."
    else
        echo "Initialising odoo_local (base module, no demo data)..."
        docker compose -p "$PROJECT" -f "$COMPOSE_FILE" \
          run --rm odoo odoo -d odoo_local -i base --without-demo=all --stop-after-init
        echo "odoo_local initialised."
    fi
    # Why: not "nothing to do" on an existing database. The demo-data cleanup
    # that archived `admin` in production was applied here too, and an archived
    # admin cannot authenticate over RPC — which is exactly the credential
    # `just e2e-live` signs in with, so Tier B failed with "no Odoo at
    # http://localhost:8070 or bad admin credential" on a perfectly healthy
    # Odoo. Re-assert the local dev admin every run; it is idempotent, and this
    # database is a local fixture, never a real one.
    echo "Ensuring the local admin is active with the dev password..."
    docker compose -p "$PROJECT" -f "$COMPOSE_FILE" exec -T odoo \
      odoo shell -d odoo_local --db_host=postgres --db_user=odoo --db_password=odoo \
        --no-http --log-level=warn <<'ODOO_SHELL'
    u = env['res.users'].with_context(active_test=False).search([('login', '=', 'admin')], limit=1)
    if u:
        u.write({'active': True, 'password': 'admin'})
        env.cr.commit()
        print('local Odoo admin is active. Login: admin / admin.')
    else:
        print('WARNING: no `admin` user in odoo_local; e2e-live will not authenticate.')
    ODOO_SHELL

# Sync the PRODUCTION Odoo (database + filestore) down onto the local sidecar.
# One command, safe to re-run: it dumps prod read-only over `flyctl ssh`, then
# REPLACES the local `odoo_local` database and filestore with that copy and
# neutralises it (crons off, mail servers removed, admin/admin restored) so a
# dev clone can never mail a real customer. Prod is never written to.
# By design this UNLINKS local Odoo identities whose stored credential the
# restored database no longer accepts — the keys they referenced are gone with
# the old database, and a row left behind claims to work and fails at the first
# tool call instead of offering the relink control.
# Needs `just db-up` (and `just odoo-local-init` once, for the odoo role).
odoo-sync-local:
    #!/usr/bin/env bash
    set -euo pipefail
    COMPOSE_FILE=".systemprompt/docker/local.yaml"
    PROJECT="$(just _project_name local)"
    COMPOSE=(docker compose -p "$PROJECT" -f "$COMPOSE_FILE")
    PG_PORT=$(sed -n 's/^ *- *"\([0-9]*\):5432"/\1/p' "$COMPOSE_FILE" | head -n1)
    [ -n "$PG_PORT" ] || { echo "ERROR: could not read the Postgres host port from $COMPOSE_FILE" >&2; exit 1; }
    STAMP=$(date +%Y%m%d_%H%M%S)
    mkdir -p backups/odoo
    DUMP="backups/odoo/prod-sync_${STAMP}.dump"
    FS="backups/odoo/prod-sync_${STAMP}-filestore.tar.gz"

    # Dump on the Fly machine: the DB password lives in its /tmp/odoo.conf and
    # is a Fly secret we cannot read from here.
    echo "==> Dumping production database + filestore on {{ODOO_APP}}..."
    # base64 so the whole script crosses `ssh -C` as one argument-safe blob.
    SCRIPT=$(base64 -w0 < deploy/fly/odoo/dump-for-sync.sh 2>/dev/null || base64 < deploy/fly/odoo/dump-for-sync.sh | tr -d '\n')
    flyctl ssh console -a {{ODOO_APP}} -C "/bin/bash -lc \"echo $SCRIPT | base64 -d | bash\""
    flyctl ssh sftp get -a {{ODOO_APP}} /tmp/sync.dump "$DUMP"
    flyctl ssh sftp get -a {{ODOO_APP}} /tmp/sync-filestore.tar.gz "$FS"
    flyctl ssh console -a {{ODOO_APP}} -C "/bin/bash -lc 'rm -f /tmp/sync.dump /tmp/sync-filestore.tar.gz'"
    echo "    saved $DUMP and $FS"

    echo "==> Replacing the local odoo_local database..."
    "${COMPOSE[@]}" stop odoo >/dev/null
    PGPASSWORD=123 psql -h localhost -p "$PG_PORT" -U systemprompt -d systemprompt -v ON_ERROR_STOP=1 <<'SQL'
    SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = 'odoo_local';
    DROP DATABASE IF EXISTS odoo_local;
    CREATE DATABASE odoo_local OWNER odoo;
    SQL
    # Production runs a newer Postgres than this box may have on PATH, and an
    # older pg_restore refuses the dump outright ("unsupported version in file
    # header"), so always reach for the newest client installed.
    PG_RESTORE=$(ls -1d /usr/lib/postgresql/*/bin/pg_restore 2>/dev/null | sort -V | tail -n1)
    [ -n "$PG_RESTORE" ] || PG_RESTORE=$(command -v pg_restore)
    # Restore as `odoo` so every object is owned by the role Odoo connects as.
    PGPASSWORD=odoo "$PG_RESTORE" -h localhost -p "$PG_PORT" -U odoo -d odoo_local \
      --no-owner --no-acl --no-privileges "$DUMP"

    echo "==> Replacing the local filestore..."
    "${COMPOSE[@]}" run --rm -T --entrypoint bash odoo -c \
      "rm -rf /var/lib/odoo/filestore/odoo_local /var/lib/odoo/filestore/{{ODOO_DB_NAME}} \
       && tar xzf - -C /var/lib/odoo \
       && mv /var/lib/odoo/filestore/{{ODOO_DB_NAME}} /var/lib/odoo/filestore/odoo_local \
       && chown -R odoo:odoo /var/lib/odoo/filestore/odoo_local" < "$FS"

    echo "==> Neutralising the clone (crons off, mail servers removed, admin/admin)..."
    "${COMPOSE[@]}" run --rm -T odoo odoo shell -d odoo_local \
      --db_host=postgres --db_user=odoo --db_password=odoo --no-http --log-level=warn \
      < deploy/fly/odoo/localize-sync.py

    echo "==> Regenerating asset bundles..."
    "${COMPOSE[@]}" run --rm -T odoo odoo shell -d odoo_local \
      --db_host=postgres --db_user=odoo --db_password=odoo --no-http --log-level=warn \
      < deploy/fly/odoo/pregenerate-assets.py

    "${COMPOSE[@]}" start odoo >/dev/null

    # The restore replaced res_users and res_users_apikeys, but `odoo_identity`
    # lives in the systemprompt database and was not touched — so every stored
    # credential now points at a row that no longer exists. Clear the dead ones
    # rather than leave accounts that look linked and fail at the first tool
    # call. See scripts/reconcile-odoo-identities.py.
    echo "==> Reconciling stored Odoo credentials against the restored database..."
    python3 scripts/reconcile-odoo-identities.py

    echo "Local Odoo now mirrors production. Login: admin / admin."

# Tail the local Odoo sidecar's logs (it starts/stops with `just db-up`/`db-down`)
odoo-local-logs:
    docker compose -p "$(just _project_name local)" -f .systemprompt/docker/local.yaml logs -f odoo

# Restart the local Odoo sidecar
odoo-local-restart:
    docker compose -p "$(just _project_name local)" -f .systemprompt/docker/local.yaml restart odoo

# Screenshot the web-tree half of the Windows-native shell (bridge review 04).
# The native chrome — title bar, tray, toasts, logon task — cannot appear here.
# Evidence for bridge-review doc 01 (navigation and IA).
bridge-nav-shots PORT="4313":
    #!/usr/bin/env bash
    set -euo pipefail
    CORE="{{justfile_directory()}}/../systemprompt-core/bin/bridge"
    cargo build --manifest-path "$CORE/Cargo.toml" --features dev-preview \
        --bin systemprompt-bridge
    if [ ! -d playwright/node_modules ]; then
        just e2e-install
    fi
    "$CORE/target/debug/systemprompt-bridge" dev-web --port {{PORT}} --web-root "$CORE/web" &
    PREVIEW_PID=$!
    trap 'kill $PREVIEW_PID 2>/dev/null || true' EXIT
    for _ in $(seq 1 40); do
        curl -sf "http://127.0.0.1:{{PORT}}/dev/fixtures" >/dev/null && break
        sleep 0.25
    done
    cd playwright && BRIDGE_PREVIEW_URL="http://127.0.0.1:{{PORT}}" \
        npx playwright test tests/bridge-navigation.spec.ts

bridge-windows-shots PORT="4312":
    #!/usr/bin/env bash
    set -euo pipefail
    CORE="{{justfile_directory()}}/../systemprompt-core/bin/bridge"
    cargo build --manifest-path "$CORE/Cargo.toml" --features dev-preview \
        --bin systemprompt-bridge
    if [ ! -d playwright/node_modules ]; then
        just e2e-install
    fi
    "$CORE/target/debug/systemprompt-bridge" dev-web --port {{PORT}} --web-root "$CORE/web" &
    PREVIEW_PID=$!
    trap 'kill $PREVIEW_PID 2>/dev/null || true' EXIT
    for _ in $(seq 1 40); do
        curl -sf "http://127.0.0.1:{{PORT}}/dev/fixtures" >/dev/null && break
        sleep 0.25
    done
    cd playwright && BRIDGE_PREVIEW_URL="http://127.0.0.1:{{PORT}}" \
        npx playwright test tests/bridge-windows-shell.spec.ts

# Print a short-lived login link for an active user on a local development profile.
dev-login USER:
    #!/usr/bin/env bash
    set -euo pipefail
    export SYSTEMPROMPT_PROFILE="${SYSTEMPROMPT_PROFILE:-{{justfile_directory()}}/.systemprompt/profiles/local/profile.yaml}"
    exec {{CLI}} plugins run dev-login "{{USER}}"

# Focused functional regression checks for shared dashboard changes.
test-dashboard stage="all":
    @scripts/build-coordinator.sh run test-dashboard "{{stage}}" -- bash scripts/test-dashboard.sh "{{stage}}"
