#!/bin/bash
# UNSEED REPORT DATA — remove everything 01-seed-report-data.sh created.
#
# The restore is exact rather than approximate because the seed is insert-only:
# every row it wrote carries an `rptseed-` id, and it updates nothing that was
# already there. So deleting that prefix returns the database to the state it
# was in before, with no reconciliation step and nothing to remember.
#
# Deletes run parent-last so no foreign key is ever left dangling, and the
# script verifies afterwards: if any prefixed row survives, it exits non-zero
# rather than reporting a clean restore it did not achieve.
#
# Safe to run when nothing is seeded — every DELETE simply matches no rows.
#
# Usage: 02-unseed-report-data.sh [--quiet]

set -e

source "$(cd "$(dirname "$0")/.." && pwd)/_common.sh"

QUIET=0
[[ "${1:-}" == "--quiet" ]] && QUIET=1

[[ $QUIET -eq 0 ]] && header "UNSEED REPORT DATA" "Restore the database to its pre-seed state"

DB_URL="$(grep database_url "$PROJECT_DIR/.systemprompt/profiles/$PROFILE/secrets.json" 2>/dev/null \
  | head -1 \
  | sed 's/.*"database_url".*"\(postgres[^"]*\)".*/\1/')"

if [[ -z "$DB_URL" ]] || ! command -v psql >/dev/null 2>&1; then
  fail "psql or database_url unavailable — cannot unseed report data"
  exit 1
fi

BEFORE=$(psql "$DB_URL" -t -A -c "SELECT COUNT(*) FROM ai_requests WHERE id LIKE 'rptseed-%';")

psql "$DB_URL" -v ON_ERROR_STOP=1 -q <<'SQL' > /dev/null
BEGIN;
DELETE FROM ai_requests          WHERE id LIKE 'rptseed-%';
DELETE FROM user_profile_ext     WHERE user_id LIKE 'rptseed-%';
DELETE FROM organization_members WHERE user_id LIKE 'rptseed-%';
DELETE FROM departments          WHERE id LIKE 'rptseed-%';
DELETE FROM organizations        WHERE id LIKE 'rptseed-%';
DELETE FROM users                WHERE id LIKE 'rptseed-%';
DELETE FROM plans                WHERE id LIKE 'rptseed-%';
COMMIT;
SQL

LEFTOVER=$(psql "$DB_URL" -t -A -c "
  SELECT COALESCE(SUM(n), 0) FROM (
    SELECT COUNT(*) AS n FROM ai_requests          WHERE id LIKE 'rptseed-%'
    UNION ALL SELECT COUNT(*) FROM user_profile_ext     WHERE user_id LIKE 'rptseed-%'
    UNION ALL SELECT COUNT(*) FROM organization_members WHERE user_id LIKE 'rptseed-%'
    UNION ALL SELECT COUNT(*) FROM departments          WHERE id LIKE 'rptseed-%'
    UNION ALL SELECT COUNT(*) FROM organizations        WHERE id LIKE 'rptseed-%'
    UNION ALL SELECT COUNT(*) FROM users                WHERE id LIKE 'rptseed-%'
    UNION ALL SELECT COUNT(*) FROM plans                WHERE id LIKE 'rptseed-%'
  ) t;")

if [[ "${LEFTOVER//[[:space:]]/}" != "0" ]]; then
  fail "$LEFTOVER seeded row(s) survived the unseed — the database is NOT restored"
  exit 1
fi

if [[ $QUIET -eq 0 ]]; then
  pass "Removed $BEFORE seeded requests and every organization, user, and plan behind them"
  pass "Verified: no rptseed- rows remain"
fi
