#!/bin/bash
# DEMO: SKILL USAGE SEED — fills the /admin/demo dashboards with real telemetry
#
# Runs the `live_demo_seed` end-to-end test against the RUNNING local stack.
# Both roles (ed@ admin, ed+notadmin@ user) sign in with PKCE, mint a hook
# token the honest way (bridge oauth-client → client_credentials, audience=hook),
# then drive Claude Code hook sessions through /api/public/hooks/track and
# /api/public/hooks/govern. Nothing is INSERTed: every row the demo pages read
# comes from the ingestion path they visualise.
#
# Cost: a handful of tiny Haiku calls through the gateway (well under $0.01)

set -euo pipefail

source "$(cd "$(dirname "$0")/.." && pwd)/_common.sh"

header "DEMO DASHBOARDS: SEED" "Real hook + gateway telemetry for two roles"

BASE="${E2E_BASE_URL:-$BASE_URL}"
export E2E_BASE_URL="$BASE"

if ! curl -sf -o /dev/null "$BASE/health"; then
  fail "No server at $BASE — run \`just start\` (or set E2E_BASE_URL)"
  exit 1
fi
pass "Server responding at $BASE"

# The seed asserts on the rows it just produced, so it needs the RUNNING
# server's own database, not a throwaway one.
if [[ -z "${DEMO_SEED_DATABASE_URL:-}" ]]; then
  SECRETS="$PROJECT_DIR/.systemprompt/profiles/$PROFILE/secrets.json"
  if [[ -f "$SECRETS" ]]; then
    DEMO_SEED_DATABASE_URL=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1])).get('database_url',''))" "$SECRETS")
    export DEMO_SEED_DATABASE_URL
  fi
fi
if [[ -z "${DEMO_SEED_DATABASE_URL:-}" ]]; then
  fail "No database_url in the $PROFILE profile — export DEMO_SEED_DATABASE_URL yourself"
  exit 1
fi
info "Asserting against ${DEMO_SEED_DATABASE_URL%%:*}://…"
echo ""

cmd "cargo nextest run -p e2e-tests --features live -E 'test(live_demo_seed)'"
cd "$PROJECT_DIR"
cargo nextest run \
  --manifest-path tests/Cargo.toml \
  -p e2e-tests \
  --features live \
  -E 'test(live_demo_seed)' \
  --no-capture

echo ""
pass "Seed complete — open $ADMIN_URL/admin/demo"
header "SKILL USAGE SEED COMPLETE"
