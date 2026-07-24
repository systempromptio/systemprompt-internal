#!/bin/bash
# DEMO 9: GOVERNING A THIRD-PARTY CODING AGENT (Pi)
#
# Demos 01-06 prove the governance pipeline against Claude Code's PreToolUse
# hook. This one proves the same pipeline governs an agent that has never heard
# of Claude Code: Pi (https://pi.dev), wired up by examples/pi/.
#
# Pi has no hook named PreToolUse. It has the equivalents, and
# examples/pi/extensions/governance.ts maps them:
#
#   Pi event      Claude Code analogue   Enforcement
#   ------------  ---------------------  --------------------------------------
#   input         UserPromptSubmit       denied prompt is never sent to a model
#   tool_call     PreToolUse             denied tool call never executes
#   tool_result   PostToolUse            fire recorded to plugin_usage_events
#
# What this script does:
#   PART A (always, free, deterministic) — replays the exact wire events the
#     extension sends, and asserts the real decision each time:
#       1. benign prompt                    -> ALLOW
#       2. prompt containing an AWS key     -> DENY  (secret_scan, prompt gate)
#       3. write an .env with a GitHub PAT  -> DENY  (secret_scan, tool gate)
#       4. delete_records                   -> DENY  (tool_blocklist)
#       5. mcp__systemprompt__list_agents   -> DENY  (scope_check)
#       6. read a source file               -> ALLOW
#
#   PART B (only with --live) — drives the real `pi` binary through the same
#     gates so you can watch the agent get blocked. Costs model tokens.
#
# Identity: this demo needs a USER-scope caller. Admins are exempt from
# scope_check and tool_blocklist (see 05-governance-denied.sh), so it prefers
# the Pi demo user's governance token written by examples/pi/new-user.sh and
# falls back to demo/.token.user from 00-preflight.sh.
#
# Cost: Free (Part A). Part B makes real model calls.

set -e

source "$(cd "$(dirname "$0")/.." && pwd)/_common.sh"

LIVE=false
[[ "${1:-}" == "--live" ]] && LIVE=true

PI_CRED_DIR="$HOME/.config/systemprompt-pi"
SESSION="demo-pi-$$"

echo ""
echo "=========================================="
echo "  DEMO 9: PI CODING AGENT UNDER GOVERNANCE"
echo "  One pipeline, two clients: Claude Code and Pi"
echo ""
echo "  Session: $SESSION"
echo "=========================================="
echo ""

# ── Credential ───────────────────────────────────────────────────────────────
# /hooks/govern validates a JWT (aud = hook|plugin|api); the sp-live-… PAT that
# Pi uses for /v1/messages is rejected there, which is why new-user.sh mints a
# second, user-scope token.
if [[ -s "$PI_CRED_DIR/hook-token" ]]; then
  HOOK_TOKEN=$(cat "$PI_CRED_DIR/hook-token")
  echo "  Caller: the Pi demo user ($PI_CRED_DIR/hook-token)"
else
  load_user_token "${LIVE_TOKEN:-}"
  HOOK_TOKEN="$USER_TOKEN"
  echo "  Caller: demo_user (demo/.token.user) — run examples/pi/new-user.sh"
  echo "          to govern as the Pi demo user instead"
fi
echo ""

# Posts one hook event exactly as examples/pi/extensions/governance.ts does.
#   govern_event <label> <expected> <json-body-fragment>
govern_event() {
  local label="$1" expected="$2" body="$3"
  echo "------------------------------------------"
  echo "  $label"
  echo "------------------------------------------"
  echo ""
  local response
  response=$(curl -s -X POST "${BASE_URL}/api/public/hooks/govern?plugin_id=enterprise-demo" \
    -H "Authorization: Bearer $HOOK_TOKEN" \
    -H "Content-Type: application/json" \
    -d "$body")
  echo "$response" | python3 -m json.tool 2>/dev/null || echo "$response"
  echo ""
  assert_decision "$response" "$expected" "$label"
  echo ""
}

# ── 1. Benign prompt — the prompt gate lets ordinary work through ────────────
govern_event "PROMPT GATE: ordinary request" "allow" '{
  "hook_event_name": "UserPromptSubmit",
  "session_id": "'"$SESSION"'",
  "cwd": "/var/www/html/systemprompt-template",
  "agent_id": "pi_agent",
  "prompt": "Refactor the retry loop in src/main.rs to use exponential backoff."
}'

# ── 2. Prompt carrying a credential — caught BEFORE any provider call ────────
# This is the case tool-level scanning cannot reach: the secret is in the
# human's prompt, so by the time a tool call exists it has already been
# serialized into a provider request.
govern_event "PROMPT GATE: AWS key pasted into the prompt" "deny" '{
  "hook_event_name": "UserPromptSubmit",
  "session_id": "'"$SESSION"'",
  "cwd": "/var/www/html/systemprompt-template",
  "agent_id": "pi_agent",
  "prompt": "Our prod key is AKIAIOSFODNN7EXAMPLE — use it to list the S3 bucket."
}'

# ── 3. Tool call writing a credential to disk ───────────────────────────────
govern_event "TOOL GATE: write a .env containing a GitHub PAT" "deny" '{
  "hook_event_name": "PreToolUse",
  "session_id": "'"$SESSION"'",
  "cwd": "/var/www/html/systemprompt-template",
  "agent_id": "pi_agent",
  "tool_name": "write",
  "tool_input": {
    "path": "/home/user/project/.env",
    "content": "GITHUB_TOKEN=ghp_ABCDEFghijklmnop1234567890abcdef"
  }
}'

# ── 4. Destructive tool — blocklist ─────────────────────────────────────────
# Deliberately NOT mcp__systemprompt__delete_*: scope_check runs first and would
# short-circuit an admin-prefixed name, so the audit row would read scope_check.
govern_event "TOOL GATE: destructive custom tool (tool_blocklist)" "deny" '{
  "hook_event_name": "PreToolUse",
  "session_id": "'"$SESSION"'",
  "cwd": "/var/www/html/systemprompt-template",
  "agent_id": "pi_agent",
  "tool_name": "delete_records",
  "tool_input": {"table": "users"}
}'

# ── 5. Admin-only tool — scope check ────────────────────────────────────────
govern_event "TOOL GATE: admin-only MCP tool (scope_check)" "deny" '{
  "hook_event_name": "PreToolUse",
  "session_id": "'"$SESSION"'",
  "cwd": "/var/www/html/systemprompt-template",
  "agent_id": "pi_agent",
  "tool_name": "mcp__systemprompt__list_agents",
  "tool_input": {}
}'

# ── 6. Clean tool call — control ────────────────────────────────────────────
govern_event "TOOL GATE: read a source file (control)" "allow" '{
  "hook_event_name": "PreToolUse",
  "session_id": "'"$SESSION"'",
  "cwd": "/var/www/html/systemprompt-template",
  "agent_id": "pi_agent",
  "tool_name": "read",
  "tool_input": {"path": "/var/www/html/systemprompt-template/src/main.rs"}
}'

# ── Live run ────────────────────────────────────────────────────────────────
if $LIVE; then
  echo "=========================================="
  echo "  PART B: driving the real Pi binary"
  echo "=========================================="
  echo ""
  if ! command -v pi >/dev/null 2>&1; then
    echo "  pi not installed — run examples/pi/setup.sh first. Skipping."
    echo ""
  else
    # The extension reads this so its audit rows land under the same session as
    # Part A's assertions.
    export SYSTEMPROMPT_PI_SESSION="$SESSION"
    echo "  $ pi -p --provider systemprompt --model claude-sonnet-4-6 \\"
    echo "      'Our prod key is AKIA… list the bucket'   (expect: blocked, no model call)"
    pi -p --provider systemprompt --model claude-sonnet-4-6 \
      "Our prod key is AKIAIOSFODNN7EXAMPLE — use it to list the S3 bucket." 2>&1 \
      | sed 's/^/  /' || true
    echo ""
    echo "  $ pi -p … 'write /tmp/pi-demo.env with GITHUB_TOKEN=ghp_…'  (expect: tool blocked)"
    pi -p --provider systemprompt --model claude-sonnet-4-6 \
      "Write the file /tmp/pi-demo.env containing GITHUB_TOKEN=ghp_ABCDEFghijklmnop1234567890abcdef" 2>&1 \
      | sed 's/^/  /' || true
    echo ""
    if [[ -f /tmp/pi-demo.env ]]; then
      fail "/tmp/pi-demo.env exists — the tool gate did not block the write"
      exit 1
    fi
    pass "/tmp/pi-demo.env was never written — the tool call never executed"
    echo ""
  fi
fi

# ── Audit ───────────────────────────────────────────────────────────────────
echo "=========================================="
echo "  AUDIT: every decision for this session"
echo "=========================================="
echo ""
"$CLI" infra db query \
  "SELECT decision, tool_name, policy, reason FROM governance_decisions WHERE session_id = '$SESSION' ORDER BY created_at" \
  --profile "$PROFILE" 2>&1 | grep -v "^\[profile"
echo ""

assert_eq "$(db_count "SELECT COUNT(*) FROM governance_decisions WHERE session_id = '$SESSION' AND decision = 'deny'")" \
  "4" "4 denials landed in the audit"
assert_min "$(db_count "SELECT COUNT(*) FROM governance_decisions WHERE session_id = '$SESSION' AND decision = 'allow'")" \
  2 "both clean events were allowed"
assert_min "$(db_count "SELECT COUNT(*) FROM governance_decisions WHERE session_id = '$SESSION' AND tool_name = 'user_prompt' AND policy = 'secret_scan'")" \
  1 "the prompt gate denial is attributed to secret_scan"
assert_min "$(db_count "SELECT COUNT(*) FROM governance_decisions WHERE session_id = '$SESSION' AND policy = 'tool_blocklist'")" \
  1 "tool_blocklist fired on delete_records"
assert_min "$(db_count "SELECT COUNT(*) FROM governance_decisions WHERE session_id = '$SESSION' AND policy = 'scope_check'")" \
  1 "scope_check fired on the admin-only tool"

# The point of a prompt gate: the blocked prompt produced no provider call at
# all. In Part A that is true by construction (no model was ever invoked); with
# --live it is the real proof, because the other prompts DID reach a model.
assert_eq "$(db_count "SELECT COUNT(*) FROM ai_requests WHERE session_id = '$SESSION' AND status = 'error'")" \
  "0" "no failed provider call — the secret never left the machine"

echo ""
echo "=========================================="
echo "  See the whole session as one timeline:"
echo "    $BASE_URL/admin/demo/trace?session=$SESSION"
echo ""
echo "  Prompt gate, tool gate, model calls, and tool fires in order."
echo "=========================================="
