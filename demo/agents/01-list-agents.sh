#!/bin/bash
# AGENTS: DISCOVERY — List and inspect configured A2A agents
#
# This instance ships NO A2A agents. services/config/config.yaml says so
# explicitly: nothing under services/agents/, nothing spawned on the agent port
# range, and no agents/<id>.md in any plugin bundle. It ships skills, MCP
# servers and artifacts instead.
#
# So this demo reads the registry rather than asserting two hard-coded names:
# it proves the discovery surface answers, and inspects whatever is configured.
# Add an agent under services/agents/ and it appears here with no edit.
#
# Cost: Free (read-only CLI commands)
#
# Usage:
#   ./demo/agents/01-list-agents.sh

set -e

source "$(cd "$(dirname "$0")/.." && pwd)/_common.sh"

header "AGENTS: DISCOVERY" "Read the A2A registry and inspect whatever it holds"

subheader "STEP 1: List All Agents"
run_cli_indented admin agents list

# The registry must answer with a well-formed list. Its length is reported, not
# demanded: zero is the correct answer for this installation.
AGENTS_JSON="$(cli_json admin agents list)"
COUNT="$(printf '%s' "$AGENTS_JSON" | jq '.items | length')"
assert_nonempty "$COUNT" "agent registry returned a structured list"
echo "  agents configured: $COUNT"
echo ""

if [[ "$COUNT" -eq 0 ]]; then
  echo "  This installation ships no A2A agents — see the note in"
  echo "  services/config/config.yaml. Skills, MCP servers and artifacts"
  echo "  carry the capability instead:"
  echo ""
  echo "      systemprompt core skills list"
  echo "      systemprompt plugins mcp list"
  echo ""
  header "AGENT DISCOVERY DEMO COMPLETE"
  exit 0
fi

subheader "STEP 2: Agent Process Status"
run_cli_indented admin agents status

# Inspect each agent the registry actually reported.
while IFS= read -r agent; do
  [[ -z "$agent" ]] && continue
  subheader "Show $agent"
  run_cli_head 30 admin agents show "$agent"
done < <(printf '%s' "$AGENTS_JSON" | jq -r '.items[] | (.agent_id // .id // .name) // empty')

header "AGENT DISCOVERY DEMO COMPLETE"
