#!/bin/bash
# FIXTURE ONLY — bypasses the hook pipeline.
#
# Writes backdated plugin_usage_events straight into Postgres so the 14-day
# charts on /admin/demo/skills and /admin/demo/tools have history behind
# today's live rows. These rows prove nothing about ingestion: they never
# touched /api/public/hooks/track, carry no governance decision, and have no
# ai_requests to attribute. 07-skill-usage-seed.sh is the honest path and is
# the one the demo runs; this exists only to make a fresh install's chart look
# like a workspace that has been in use.
#
# Not run by default. Usage: ./demo/skills/07b-skill-usage-backfill.sh [days]
#
# Cost: Free

set -euo pipefail

source "$(cd "$(dirname "$0")/.." && pwd)/_common.sh"

DAYS="${1:-14}"

header "DEMO DASHBOARDS: BACKFILL" "FIXTURE ONLY — ${DAYS} days of synthetic history"

warn "These rows bypass the hook pipeline. Do not present them as ingestion evidence."
echo ""

if [[ -z "${DEMO_SEED_DATABASE_URL:-}" ]]; then
  SECRETS="$PROJECT_DIR/.systemprompt/profiles/$PROFILE/secrets.json"
  if [[ -f "$SECRETS" ]]; then
    DEMO_SEED_DATABASE_URL=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1])).get('database_url',''))" "$SECRETS")
  fi
fi
if [[ -z "${DEMO_SEED_DATABASE_URL:-}" ]]; then
  fail "No database_url in the $PROFILE profile — export DEMO_SEED_DATABASE_URL yourself"
  exit 1
fi
if ! command -v psql >/dev/null 2>&1; then
  fail "psql not found — install the postgres client, or run this inside the db container"
  exit 1
fi

ADMIN_EMAIL="${DEMO_ADMIN_LOGIN:-ed@systemprompt.io}"
USER_EMAIL="${DEMO_USER_LOGIN:-ed+notadmin@systemprompt.io}"
info "admin=$ADMIN_EMAIL  user=$USER_EMAIL  days=$DAYS"
echo ""

# One statement: a row per (user, skill/tool, day) with a deterministic-looking
# but varied count, skipping any user the install does not have.
psql "$DEMO_SEED_DATABASE_URL" -v ON_ERROR_STOP=1 \
  -v days="$DAYS" -v admin_email="$ADMIN_EMAIL" -v user_email="$USER_EMAIL" <<'SQL'
WITH actors(email, tool, skill) AS (
    VALUES
        (:'admin_email', 'Skill', 'systemprompt-admin:admin-user-report'),
        (:'admin_email', 'Skill', 'systemprompt-admin:admin-activity-report'),
        (:'admin_email', 'mcp__odoo__crm_lead_search', NULL),
        (:'user_email',  'Skill', 'systemprompt-business:manage_leads'),
        (:'user_email',  'mcp__odoo__crm_lead_search', NULL)
),
days AS (SELECT generate_series(1, :days::int) AS ago),
reps AS (SELECT generate_series(1, 3) AS n)
INSERT INTO plugin_usage_events
    (id, user_id, session_id, event_type, tool_name, plugin_id, metadata, created_at)
SELECT
    gen_random_uuid()::text,
    u.id,
    'backfill-' || d.ago || '-' || md5(a.email || a.tool || r.n::text),
    'PostToolUse',
    a.tool,
    CASE WHEN a.skill IS NULL THEN 'systemprompt-business'
         ELSE split_part(a.skill, ':', 1) END,
    CASE WHEN a.skill IS NULL THEN '{}'::jsonb
         ELSE jsonb_build_object('tool_input', jsonb_build_object('skill', a.skill)) END,
    now() - make_interval(days => d.ago, hours => 9 + r.n)
FROM actors a
JOIN users u ON lower(u.email) = lower(a.email)
CROSS JOIN days d
CROSS JOIN reps r
WHERE (d.ago + r.n) % 2 = 0;
SQL

echo ""
pass "Backfill written — reload $ADMIN_URL/admin/demo/skills"
header "BACKFILL COMPLETE (FIXTURE DATA)"
