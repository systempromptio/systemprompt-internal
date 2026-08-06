#!/bin/bash
# DEMO 2: THE SCOPED PATH — EVERY CALL IS IDENTIFIED AND AUDITED
# A genuinely user-scope caller invokes an admin-only tool. This script asserts
# the real decision rather than narrating one — and in THIS installation the
# real decision is `allow`, because governance is switched off.
#
# Identity model (the honest part):
#   Governance derives access scope from the CALLER'S LIVE DB ROLES, not from
#   the agent_id in the payload. So we send the request with the user-scope
#   plugin token from demo/.token.user (minted by 00-preflight.sh for
#   demo_user@demo.local, whose DB role is `user`). That token resolves to User
#   scope — which is what gets recorded against the call.
#
# What this proves HERE:
#   Governance is disabled (all four stages `enabled: false` in
#   services/governance/config.yaml), so scope_check does not run and nothing
#   is refused. What still holds — and what this demo asserts — is that the
#   call reaches the audit spine with its identity, session and tool attached,
#   recorded as decision=allow, policy=governance_disabled.
#
#   With the four stages enabled, this same call is denied by scope_check,
#   because mcp__systemprompt__ is listed under admin_only_prefixes.
#
# What this does:
#   1. POST /api/public/hooks/govern with the user-scope token, simulating a
#      PreToolUse hook for associate_agent calling mcp__systemprompt__list_agents
#   2. Asserts permissionDecision == allow (the disabled-chain behaviour)
#   3. Prints commentary on defense-in-depth (mapping + rules)
#   4. Asserts the decision landed in governance_decisions
#
# Flow:
#   curl → POST /hooks/govern → JWT auth → DB role=user → chain disabled → ALLOW + audit
#
# Cost: Free (no AI call)

set -e

source "$(cd "$(dirname "$0")/.." && pwd)/_common.sh"

echo ""
echo "=========================================="
echo "  DEMO 2: THE SCOPED PATH"
echo "  user-scope token calls an admin-only tool"
echo "=========================================="
echo ""

# ──────────────────────────────────────────────
#  Load the user-scope auth token (real User scope, DB-role derived)
# ──────────────────────────────────────────────
load_user_token "${1:-}"

# ──────────────────────────────────────────────
#  PART 1: A user-scope caller invokes an admin-only tool
# ──────────────────────────────────────────────
echo "------------------------------------------"
echo "  Simulating PreToolUse hook:"
echo "  identity=demo_user (user scope, token-derived from DB role)"
echo "  agent=associate_agent (user scope)"
echo "  tool=mcp__systemprompt__list_agents"
echo "------------------------------------------"
echo ""

RESPONSE=$(curl -s -X POST "${BASE_URL}/api/public/hooks/govern?plugin_id=enterprise-demo" \
  -H "Authorization: Bearer $USER_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "hook_event_name": "PreToolUse",
    "tool_name": "mcp__systemprompt__list_agents",
    "agent_id": "associate_agent",
    "session_id": "demo-refused-path",
    "cwd": "/var/www/html/systemprompt-template",
    "tool_input": {}
  }')
echo "$RESPONSE" | python3 -m json.tool 2>/dev/null || echo "(Could not pretty-print response)"
echo ""
assert_decision "$RESPONSE" "allow" "governance disabled — the call is allowed and audited"
governance_disabled_note

# ──────────────────────────────────────────────
#  PART 2: Defense-in-depth commentary
# ──────────────────────────────────────────────
echo ""
echo "=========================================="
echo "  DEFENSE-IN-DEPTH"
echo "=========================================="
echo ""
echo "  Two independent layers prevent unauthorized access:"
echo ""
echo "  Layer 1 — MAPPING (preventive)"
echo "    In a real Claude Code deployment, user-scope agents"
echo "    never even see admin tools. The MCP server mapping"
echo "    excludes them entirely — the tool does not appear"
echo "    in the agent's tool list."
echo ""
echo "  Layer 2 — GOVERNANCE RULES (detective + enforcement)"
echo "    When enabled, the scope_check stage evaluates every"
echo "    PreToolUse hook call, and a user-scope agent calling an"
echo "    admin tool is denied and logged. In this installation"
echo "    that stage is off, so the call is allowed — but it is"
echo "    still logged, with the caller's identity attached."
echo ""
echo "  Result: Two independent layers. Mapping prevents exposure."
echo "  Governance enforces policy when switched on. The audit"
echo "  spine records the call either way."
echo ""

# ──────────────────────────────────────────────
#  AUDIT: Query governance_decisions for the recorded decision
# ──────────────────────────────────────────────
echo "=========================================="
echo "  AUDIT: Governance decisions for this session"
echo "=========================================="
echo ""

echo "  Decision counts (session=demo-refused-path):"
"$CLI" infra db query \
  "SELECT decision, COUNT(*) as count FROM governance_decisions WHERE session_id = 'demo-refused-path' GROUP BY decision ORDER BY decision" \
  2>&1 | grep -v "^\[profile"

echo ""
echo "  Expected: 1 allow (policy=governance_disabled)"
echo ""
assert_governance_audited "demo-refused-path" \
  "the decision landed in the audit for demo-refused-path"
echo ""

echo "  Detailed decisions:"
"$CLI" infra db query \
  "SELECT decision, tool_name, policy, reason FROM governance_decisions WHERE session_id = 'demo-refused-path' ORDER BY created_at" \
  2>&1 | grep -v "^\[profile"

echo ""
echo "=========================================="
echo "  AUDIT COMMANDS (run manually):"
echo "  $CLI infra db query \"SELECT * FROM governance_decisions WHERE session_id = 'demo-refused-path' ORDER BY created_at\""
echo ""
echo "  associate_agent (user scope) called"
echo "  mcp__systemprompt__list_agents. Governance is disabled, so"
echo "  the call was ALLOWED and recorded as policy=governance_disabled."
echo "  Enable scope_check to have this same call denied."
echo ""
echo "  Now run: ./demo/governance/03-audit-trail.sh"
echo "=========================================="
