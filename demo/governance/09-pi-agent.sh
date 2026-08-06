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
#       4. delete_records                   -> DENY  (tool_blocklist)*
#       5. mcp__systemprompt__list_agents   -> DENY  (scope_check)*
#     *ALLOW when the acting user is an admin — see "Identity" below.
#       6. read a source file               -> ALLOW
#
#   PART B (only with --live) — drives the real `pi` binary through the same
#     gates so you can watch the agent get blocked. Costs model tokens.
#
# Identity: the caller is whoever examples/pi/new-user.sh was pointed at
# (~/.config/systemprompt-pi/), falling back to demo/.token.user from
# 00-preflight.sh. Admins are exempt from scope_check and tool_blocklist
# (scope_check.rs:66, tool_blocklist.rs:59), so cases 4 and 5 are asserted as
# ALLOW for an admin caller — that exemption IS the enforced outcome. Pick a
# non-admin user to see the denial path.
#
# Cost: Free (Part A). Part B makes real model calls.

set -e

source "$(cd "$(dirname "$0")/.." && pwd)/_common.sh"

LIVE=false
[[ "${1:-}" == "--live" ]] && LIVE=true

PI_CRED_DIR="$HOME/.config/systemprompt-pi"

# Pi credentials live in $HOME, so they outlive the instance that issued them:
# a token, hook token, and user record left by an earlier server are all
# rejected by this one. Trust the directory only when it names this instance.
if [[ ! -s "$PI_CRED_DIR/base-url" ]] \
  || [[ "$(cat "$PI_CRED_DIR/base-url")" != "$BASE_URL" ]]; then
  PI_CRED_DIR="$(mktemp -d)"
fi

# ── Session ──────────────────────────────────────────────────────────────────
# The gateway attests x-session-id against a session row it issued, so this
# demo cannot invent a per-run label any more: it obtains a real session and
# both spines (governance_decisions here, ai_requests from Part B's model
# calls) key on it. Two ways to obtain one, the same two the Pi extension uses:
# a JWT already carries its session_id claim, a PAT mints a row.
jwt_session_claim() {
  local payload pad
  [[ $(printf '%s' "$1" | awk -F. '{print NF}') -eq 3 ]] || return 0
  payload=$(printf '%s' "$1" | cut -d. -f2)
  pad=$(( (4 - ${#payload} % 4) % 4 ))
  { printf '%s' "$payload"; [[ $pad -gt 0 ]] && printf '%.0s=' $(seq 1 $pad); } \
    | tr '_-' '/+' | base64 -d 2>/dev/null \
    | sed -n 's/.*"session_id"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1
}

obtain_session() {
  local credential="$1" session
  [[ -n "$credential" ]] || return 0
  session=$(jwt_session_claim "$credential")
  if [[ -z "$session" ]]; then
    session=$(curl -fsS -X POST "${BASE_URL}/api/public/gateway/sessions" \
      -H "x-api-key: $credential" | sed -n 's/.*"session_id":"\([^"]*\)".*/\1/p')
  fi
  printf '%s' "$session"
}

CREDENTIAL=""
[[ -s "$PI_CRED_DIR/token" ]] && CREDENTIAL=$(cat "$PI_CRED_DIR/token")
SESSION=$(obtain_session "$CREDENTIAL")

if [[ -z "$SESSION" ]]; then
  load_user_token "${LIVE_TOKEN:-}"
  CREDENTIAL="$USER_TOKEN"
  SESSION=$(obtain_session "$CREDENTIAL")
fi
if [[ -z "$SESSION" ]]; then
  echo "ERROR: could not obtain a gateway session." >&2
  echo "  A PAT caller mints one at POST ${BASE_URL}/api/public/gateway/sessions;" >&2
  echo "  run examples/pi/new-user.sh (or demo/00-preflight.sh) and retry." >&2
  exit 1
fi

# A JWT caller reuses the session its token was minted with, so runs can share
# a session id. Scope every audit assertion to this run by the database clock
# (not the host's) so the counts stay exact either way.
RUN_START=$(cli_json infra db query "SELECT now() AS t" | jq -r '.items[0].t // empty')
[[ -n "$RUN_START" ]] || { echo "ERROR: could not read the database clock" >&2; exit 1; }
SINCE="AND created_at >= '$RUN_START'"

echo ""
echo "=========================================="
echo "  DEMO 9: PI CODING AGENT UNDER GOVERNANCE"
echo "  One pipeline, two clients: Claude Code and Pi"
echo ""
echo "  Session: $SESSION (issued by the gateway, not invented here)"
echo "=========================================="
echo ""

# ── Credential ───────────────────────────────────────────────────────────────
# /hooks/govern validates a JWT (aud = hook|plugin|api); the sp-live-… PAT that
# Pi uses for /v1/messages is rejected there, which is why new-user.sh mints a
# second, user-scope token.
CALLER_IS_ADMIN=0
CALLER_USER_ID=""
if [[ -s "$PI_CRED_DIR/hook-token" ]]; then
  HOOK_TOKEN=$(cat "$PI_CRED_DIR/hook-token")
  CALLER_EMAIL="the Pi user"
  CALLER_ROLES="unknown"
  if [[ -s "$PI_CRED_DIR/user.json" ]]; then
    CALLER_EMAIL=$(jq -r '.email' "$PI_CRED_DIR/user.json")
    CALLER_ROLES=$(jq -r '.roles' "$PI_CRED_DIR/user.json")
    CALLER_USER_ID=$(jq -r '.id' "$PI_CRED_DIR/user.json")
    [[ "$(jq -r '.is_admin' "$PI_CRED_DIR/user.json")" == "true" ]] && CALLER_IS_ADMIN=1
  fi
  echo "  Caller: $CALLER_EMAIL — roles: $CALLER_ROLES ($PI_CRED_DIR/hook-token)"
else
  load_user_token "${LIVE_TOKEN:-}"
  HOOK_TOKEN="$USER_TOKEN"
  echo "  Caller: demo_user (demo/.token.user) — run examples/pi/new-user.sh"
  echo "          to govern as a user you select from the database instead"
fi

# Admins are exempt from two of the four policies, so the expected decision for
# those two cases is ALLOW. Computing it here (rather than hard-coding DENY)
# keeps every case a real assertion whichever user was selected.
# Governance is DISABLED in this installation (all four stages `enabled: false`
# in services/governance/config.yaml), so no stage judges anything and every
# gate below answers ALLOW regardless of caller scope. The per-scope reasoning
# that follows is what applies once the stages are switched back on.
GOVERNANCE_DISABLED=1
PROMPT_SECRET_EXPECT="allow"
TOOL_SECRET_EXPECT="allow"

if [[ $CALLER_IS_ADMIN -eq 1 ]]; then
  SCOPED_EXPECT="allow"
  echo ""
  echo "  This caller is an admin. scope_check (scope_check.rs:66) and"
  echo "  tool_blocklist (tool_blocklist.rs:59) short-circuit on admin scope,"
  echo "  so cases 4 and 5 below are asserted as ALLOW — that exemption is the"
  echo "  enforced outcome, not a gap. secret_scan has no exemption and still"
  echo "  denies. Select a non-admin user to prove the denial path."
else
  SCOPED_EXPECT="deny"
fi

if [[ $GOVERNANCE_DISABLED -eq 1 ]]; then
  SCOPED_EXPECT="allow"
  governance_disabled_note
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
govern_event "PROMPT GATE: AWS key pasted into the prompt" "$PROMPT_SECRET_EXPECT" '{
  "hook_event_name": "UserPromptSubmit",
  "session_id": "'"$SESSION"'",
  "cwd": "/var/www/html/systemprompt-template",
  "agent_id": "pi_agent",
  "prompt": "Our prod key is AKIAIOSFODNN7EXAMPLE — use it to list the S3 bucket."
}'

# ── 3. Tool call writing a credential to disk ───────────────────────────────
govern_event "TOOL GATE: write a .env containing a GitHub PAT" "$TOOL_SECRET_EXPECT" '{
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
govern_event "TOOL GATE: destructive custom tool (tool_blocklist)" "$SCOPED_EXPECT" '{
  "hook_event_name": "PreToolUse",
  "session_id": "'"$SESSION"'",
  "cwd": "/var/www/html/systemprompt-template",
  "agent_id": "pi_agent",
  "tool_name": "delete_records",
  "tool_input": {"table": "users"}
}'

# ── 5. Admin-only tool — scope check ────────────────────────────────────────
govern_event "TOOL GATE: admin-only MCP tool (scope_check)" "$SCOPED_EXPECT" '{
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
    # The extension reads this as "use this already-minted session", so its
    # audit rows and its model calls land under the same session as Part A's
    # assertions.
    export SYSTEMPROMPT_PI_SESSION="$SESSION"

    echo "  PROMPT GATE — the credential is in the prompt itself."
    echo "  $ pi -p 'Our prod key is AKIA… list the bucket'"
    echo ""
    pi -p --provider systemprompt --model claude-sonnet-4-6 \
      "Our prod key is AKIAIOSFODNN7EXAMPLE — use it to list the S3 bucket." 2>&1 \
      | sed 's/^/  /' || true
    echo ""
    echo "  ^ No output: Pi never called a model. The prompt stopped at the gate."
    echo ""

    echo "  TOOL GATE — the model calls a destructive tool and is blocked."
    echo "  $ pi -p 'Use the delete_records tool to delete rows from the users table.'"
    echo ""
    pi -p --provider systemprompt --model claude-sonnet-4-6 \
      "Use the delete_records tool to delete rows from the users table. Just call it." 2>&1 \
      | sed 's/^/  /' || true
    echo ""
    echo "  ^ The model was handed the denial reason and had to explain it."
    echo ""

    if [[ $CALLER_IS_ADMIN -eq 0 ]]; then
      assert_min "$(db_count "SELECT COUNT(*) FROM governance_decisions WHERE session_id = '$SESSION' $SINCE AND policy = 'tool_blocklist' AND decision = 'deny'")" \
        2 "the live delete_records call was blocked too (Part A + Part B)"
    else
      echo "  (admin caller: tool_blocklist is exempt, so the live call ran)"
    fi
    echo ""

    # Why not a live "write a .env full of secrets" case? Two reasons, both
    # worth knowing about this deployment:
    #   1. The gateway's own safety policy rejects a /v1/messages request whose
    #      conversation carries a credential (403, category 'secret'), so a
    #      model-authored secret write is pre-empted upstream and never reaches
    #      the tool gate at all. Defence in depth, but it makes for a confusing
    #      demo beat.
    #   2. Frontier models frequently refuse to fabricate a realistic
    #      credential, so the tool call never happens and nothing is proven.
    # The tool gate's secret_scan is asserted deterministically in Part A
    # instead, where the tool input is presented directly.
  fi
fi

# ── Audit ───────────────────────────────────────────────────────────────────
echo "=========================================="
echo "  AUDIT: every decision for this session"
echo "=========================================="
echo ""
"$CLI" infra db query \
  "SELECT decision, tool_name, policy, reason FROM governance_decisions WHERE session_id = '$SESSION' $SINCE ORDER BY created_at" \
  --profile "$PROFILE" 2>&1 | grep -v "^\[profile"
echo ""

# Part A produces four denials for a user-scope caller, and two for an admin —
# the two secret_scan cases, the policies with no admin exemption. A --live run
# adds its own, so only the deterministic path can assert an exact count.
if [[ $CALLER_IS_ADMIN -eq 1 ]]; then
  EXPECTED_DENIES=2
  EXPECTED_ALLOWS=4
else
  EXPECTED_DENIES=4
  EXPECTED_ALLOWS=2
fi
if [[ $GOVERNANCE_DISABLED -eq 1 ]]; then
  EXPECTED_DENIES=0
  EXPECTED_ALLOWS=6
fi
DENIES=$(db_count "SELECT COUNT(*) FROM governance_decisions WHERE session_id = '$SESSION' $SINCE AND decision = 'deny'")
if $LIVE; then
  assert_min "$DENIES" "$EXPECTED_DENIES" "at least the $EXPECTED_DENIES scripted denials landed in the audit"
else
  assert_eq "$DENIES" "$EXPECTED_DENIES" "$EXPECTED_DENIES denials landed in the audit"
fi
assert_min "$(db_count "SELECT COUNT(*) FROM governance_decisions WHERE session_id = '$SESSION' $SINCE AND decision = 'allow'")" \
  "$EXPECTED_ALLOWS" "the clean events were allowed"
if [[ $GOVERNANCE_DISABLED -eq 1 ]]; then
  assert_min "$(db_count "SELECT COUNT(*) FROM governance_decisions WHERE session_id = '$SESSION' $SINCE AND policy = 'governance_disabled'")" \
    "$EXPECTED_ALLOWS" "every scripted event was audited under policy=governance_disabled"
  echo "  (chain disabled: no event is attributed to a policy stage)"
elif [[ $CALLER_IS_ADMIN -eq 0 ]]; then
  assert_min "$(db_count "SELECT COUNT(*) FROM governance_decisions WHERE session_id = '$SESSION' $SINCE AND tool_name = 'user_prompt' AND policy = 'secret_scan'")" \
    1 "the prompt gate denial is attributed to secret_scan"
  assert_min "$(db_count "SELECT COUNT(*) FROM governance_decisions WHERE session_id = '$SESSION' $SINCE AND policy = 'tool_blocklist'")" \
    1 "tool_blocklist fired on delete_records"
  assert_min "$(db_count "SELECT COUNT(*) FROM governance_decisions WHERE session_id = '$SESSION' $SINCE AND policy = 'scope_check'")" \
    1 "scope_check fired on the admin-only tool"
else
  echo "  (admin caller: tool_blocklist and scope_check are exempt by design)"
fi

# The point of a prompt gate: the blocked prompt produced no provider call at
# all. In Part A that is true by construction (no model was ever invoked); with
# --live it is the real proof, because the other prompts DID reach a model.
assert_eq "$(db_count "SELECT COUNT(*) FROM ai_requests WHERE session_id = '$SESSION' $SINCE AND status = 'error'")" \
  "0" "no failed provider call — the secret never left the machine"

# Attribution is the whole point of selecting a real user: every decision this
# run produced has to carry their user_id, or the evidence lands on nobody and
# their profile page stays empty. Assert it rather than trusting the header.
if [[ -n "$CALLER_USER_ID" ]]; then
  assert_eq "$(db_count "SELECT COUNT(*) FROM governance_decisions WHERE session_id = '$SESSION' $SINCE AND (user_id IS NULL OR user_id::text <> '$CALLER_USER_ID')")" \
    "0" "every decision is attributed to $CALLER_EMAIL"
  echo ""
  echo "  Same view from the CLI:"
  echo "    $CLI infra logs request list --user $CALLER_USER_ID"
  echo "    $BASE_URL/admin/user?id=$CALLER_USER_ID"
fi

echo ""
echo "=========================================="
echo "  See the whole session as one timeline:"
echo "    $ADMIN_URL/admin/demo/trace?session=$SESSION"
echo ""
echo "  Prompt gate, tool gate, model calls, and tool fires in order."
echo "=========================================="
