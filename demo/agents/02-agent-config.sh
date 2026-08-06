#!/bin/bash
# AGENTS: CONFIGURATION — Validation, tools, status
#
# Validates every A2A agent the registry reports and lists the MCP tools each
# one can reach. This instance ships no A2A agents (see the note in
# services/config/config.yaml), so there is nothing to validate here and the
# demo says so rather than validating two names that no longer exist.
#
# Cost: Free (read-only CLI commands)
#
# Usage:
#   ./demo/agents/02-agent-config.sh

set -e

source "$(cd "$(dirname "$0")/.." && pwd)/_common.sh"

# Show an agent's validation result (box table) and assert validity against the
# structured --json card (sections[].heading=="valid"), failing loudly otherwise.
validate_agent() {
  local agent="$1"
  cmd "systemprompt admin agents validate $agent"
  "$CLI" admin agents validate "$agent" --profile "$PROFILE" 2>&1 | sed 's/^/  /'
  echo ""
  assert_eq "$(cli_json admin agents validate "$agent" \
    | jq -r '.sections[]|select(.heading=="valid").content')" \
    "true" "$agent configuration valid"
  echo ""
}

header "AGENTS: CONFIGURATION" "Validation and MCP tool inventory per configured agent"

subheader "STEP 1: Agent Process Status"
run_cli_indented admin agents status

AGENTS_JSON="$(cli_json admin agents list)"
COUNT="$(printf '%s' "$AGENTS_JSON" | jq '.items | length')"
assert_nonempty "$COUNT" "agent registry returned a structured list"
echo "  agents configured: $COUNT"
echo ""

if [[ "$COUNT" -eq 0 ]]; then
  echo "  No A2A agents are configured, so there is no agent config to"
  echo "  validate. The capability this instance actually ships is validated"
  echo "  by its own demos:"
  echo ""
  echo "      ./demo/skills/01-skill-lifecycle.sh"
  echo "      ./demo/mcp/01-mcp-servers.sh"
  echo ""
  header "AGENT CONFIG DEMO COMPLETE"
  exit 0
fi

# Validate and inventory each agent the registry actually reported.
while IFS= read -r agent; do
  [[ -z "$agent" ]] && continue
  subheader "Validate $agent"
  validate_agent "$agent"
  subheader "MCP Tools Available to $agent"
  run_cli_head 30 admin agents tools "$agent"
done < <(printf '%s' "$AGENTS_JSON" | jq -r '.items[] | (.agent_id // .id // .name) // empty')

header "AGENT CONFIG DEMO COMPLETE"
