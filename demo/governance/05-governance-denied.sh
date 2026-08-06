#!/bin/bash
# DEMO 5: THE POLICY PATH — WHAT THE CHAIN WOULD JUDGE, AND WHAT IT RECORDS
# Sends the two calls that scope_check and tool_blocklist exist to refuse, and
# asserts the real decision rather than narrating one. Governance is DISABLED in
# this installation, so the real decision is `allow` and what this proves is the
# audit spine: both calls are recorded against the caller's identity.
#
# Identity model (the honest part):
#   Governance derives access scope from the CALLER'S LIVE DB ROLES. This script
#   sends every request with the user-scope plugin token from
#   demo/.token.user (minted by 00-preflight.sh for demo_user@demo.local, DB role
#   `user`). That token resolves to User scope — the scope recorded against each
#   call, and the scope that scope_check and tool_blocklist would judge were they
#   enabled. (Admins are exempt from both, which is why this demo uses the
#   user-scope token rather than demo/.token.)
#
# What this does:
#   Part 1 — Scope restriction denial:
#     1. Loads the user-scope token from demo/.token.user (set by 00-preflight.sh)
#     2. POSTs directly to /api/public/hooks/govern with:
#        - tool_name: mcp__systemprompt__list_agents (admin-only MCP tool)
#        - agent_id: associate_agent (user scope)
#     3. Captures the JSON response and asserts permissionDecision == allow
#        - with scope_check enabled this is denied: user scope cannot reach
#          mcp__systemprompt__* tools
#
#   Part 2 — Blocklist denial:
#     1. POSTs directly to /api/public/hooks/govern with:
#        - tool_name: delete_records (destructive name, NOT admin-prefixed)
#        - agent_id: associate_agent (user scope)
#        - tool_input: {"table":"users"}
#     2. Captures the JSON response and asserts permissionDecision == allow
#        - tool_blocklist is the policy that WOULD fire here. We use a
#          NON-admin-prefixed destructive name on purpose: an
#          mcp__systemprompt__delete_* tool would be short-circuited by
#          scope_check (it runs first), so the deny would be attributed to
#          scope_check, not tool_blocklist. delete_records passes scope_check
#          (not admin-only) and is the tool_blocklist case.
#        - tool_blocklist catches destructive names (delete/drop/destroy) for
#          user/non-admin scope (admins are exempt).
#
# What Claude Code does with a deny response (when the stages are enabled):
#   1. The PreToolUse hook returns permissionDecision: "deny" with a reason
#   2. Claude Code prints: [GOVERNANCE] <reason> — visible in the terminal
#   3. Claude Code BLOCKS the tool call — it never executes
#   4. The agent receives the denial reason and must explain it to the user
# Here the stages are off, so the hook returns "allow" and the call proceeds.
#   5. The denial is logged to governance_decisions for audit
#
# Flow:
#   Agent → MCP tool call → PreToolUse hook → POST /hooks/govern
#   → JWT auth → scope=user → scope_check FAILS → DENY → tool blocked
#
# Cost: Free (two direct API calls, no AI usage)

set -e

source "$(cd "$(dirname "$0")/.." && pwd)/_common.sh"

echo ""
echo "=========================================="
echo "  DEMO 5: GOVERNANCE — THE POLICY PATH"
echo "  demo_user — real user scope, governance disabled"
echo ""
echo "  Flow:"
echo "    1. Agent calls MCP tool"
echo "    2. PreToolUse hook fires (synchronous)"
echo "    3. Hook POSTs to /api/public/hooks/govern"
echo "    4. Backend evaluates governance rules"
echo "    5. Backend returns HTTP 200 and audits the decision"
echo "    6. Hook outputs permissionDecision=allow (chain disabled)"
echo "    7. Claude Code runs the tool; the decision is on record"
echo "=========================================="
echo ""

# Load the user-scope token from demo/.token.user (set by 00-preflight.sh).
# Governance derives scope from the caller's live DB role, so this token
# resolves to User scope — the scope the policies would judge, and the scope
# recorded in the audit either way.
load_user_token "${1:-}"

# ──────────────────────────────────────────────
#  PART 1: Scope restriction — user cannot access admin tools
# ──────────────────────────────────────────────
echo "------------------------------------------"
echo "  PART 1: Scope restriction denial"
echo "  identity: demo_user (user scope, token-derived from DB role)"
echo "  tool: mcp__systemprompt__list_agents"
echo "  agent: associate_agent (user scope)"
echo "  rule: scope_check — user scope cannot access mcp__systemprompt__* tools"
echo "------------------------------------------"
echo ""

RESPONSE=$(curl -s -X POST "${BASE_URL}/api/public/hooks/govern?plugin_id=enterprise-demo" \
  -H "Authorization: Bearer $USER_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "hook_event_name": "PreToolUse",
    "tool_name": "mcp__systemprompt__list_agents",
    "agent_id": "associate_agent",
    "session_id": "demo-governance-denied",
    "cwd": "/var/www/html/systemprompt-template"
  }')
echo "$RESPONSE" | python3 -m json.tool 2>/dev/null || echo "(Could not pretty-print response)"

echo ""
assert_decision "$RESPONSE" "allow" "governance disabled — admin-tool call allowed and audited"
echo ""

# ──────────────────────────────────────────────
#  PART 2: Blocklist — destructive tool blocked
# ──────────────────────────────────────────────
echo "------------------------------------------"
echo "  PART 2: Blocklist denial"
echo "  identity: demo_user (user scope, token-derived from DB role)"
echo "  tool: delete_records (destructive name, NOT admin-prefixed)"
echo "  agent: associate_agent (user scope)"
echo "  rule: tool_blocklist — destructive names (delete/drop/destroy) denied"
echo "        for user/non-admin scope (admins are exempt)"
echo "  Why not mcp__systemprompt__delete_*? scope_check runs first and would"
echo "  short-circuit it, attributing the deny to scope_check. A non-prefixed"
echo "  name passes scope_check and is denied by tool_blocklist — so the audit"
echo "  row genuinely reads policy=tool_blocklist."
echo "------------------------------------------"
echo ""

RESPONSE=$(curl -s -X POST "${BASE_URL}/api/public/hooks/govern?plugin_id=enterprise-demo" \
  -H "Authorization: Bearer $USER_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "hook_event_name": "PreToolUse",
    "tool_name": "delete_records",
    "tool_input": {"table": "users"},
    "agent_id": "associate_agent",
    "session_id": "demo-governance-denied-blocklist",
    "cwd": "/var/www/html/systemprompt-template"
  }')
echo "$RESPONSE" | python3 -m json.tool 2>/dev/null || echo "(Could not pretty-print response)"

echo ""
assert_decision "$RESPONSE" "allow" "governance disabled — destructive-tool call allowed and audited"
echo ""
echo "  ^ With tool_blocklist enabled this is blocked for user scope, and the"
echo "    audit row reads policy=tool_blocklist. Here the stage is off, so the"
echo "    row reads policy=governance_disabled and the call proceeds."
echo "  In Claude Code with the stage enabled, the agent would see:"
echo "    [GOVERNANCE] Tool blocked: <reason>"
echo "    The tool call never executes. The agent must explain the denial."
echo ""

# ──────────────────────────────────────────────
#  GOVERNANCE LOG
# ──────────────────────────────────────────────
echo "=========================================="
echo "  GOVERNANCE LOG — recent deny decisions"
echo "=========================================="
echo ""
"$CLI" infra db query \
  "SELECT decision, tool_name, policy, reason FROM governance_decisions WHERE decision = 'deny' ORDER BY created_at DESC LIMIT 10" \
  --profile "$PROFILE" 2>&1 | grep -v "^\[profile"

# ──────────────────────────────────────────────
#  AUDIT: Verify governance denials
# ──────────────────────────────────────────────
echo ""
echo "=========================================="
echo "  AUDIT: Governance decisions"
echo "=========================================="
echo ""

echo "  Most recent governance decisions:"
"$CLI" infra db query \
  "SELECT decision, tool_name, policy, reason FROM governance_decisions ORDER BY created_at DESC LIMIT 5" \
  2>&1 | grep -v "^\[profile"

echo ""
echo "  Expected: two allow records, both policy=governance_disabled"
echo "    1. the admin-only tool mcp__systemprompt__list_agents"
echo "    2. the destructive tool delete_records"
echo "  With the stages enabled these are two denies, attributed to"
echo "  scope_check and tool_blocklist respectively."
echo ""
assert_governance_audited "demo-governance-denied" \
  "scope call landed in audit (demo-governance-denied)"
assert_governance_audited "demo-governance-denied-blocklist" \
  "blocklist call landed in audit (demo-governance-denied-blocklist)"

echo ""
echo "=========================================="
echo "  Now run: ./demo/governance/06-secret-breach.sh"
echo "=========================================="
