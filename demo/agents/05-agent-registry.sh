#!/bin/bash
# AGENTS: REGISTRY & LOGS — A2A discovery, running agents, process logs
# Shows the A2A agent registry and per-agent process logs.
#
# Cost: Free (read-only CLI commands)
#
# Usage:
#   ./demo/agents/05-agent-registry.sh

set -e

source "$(cd "$(dirname "$0")/.." && pwd)/_common.sh"

header "AGENTS: REGISTRY & LOGS" "A2A discovery, running agents, process logs"

subheader "STEP 1: Agent Registry (A2A Gateway)"
cmd "systemprompt admin agents registry"
"$CLI" admin agents registry --profile "$PROFILE" 2>&1 | head -30 | sed 's/^/  /' || info "Registry unavailable — agents may need restart."
echo ""

# Per-agent logs, for whatever the registry reports. Naming agents here meant
# the demo died on `agents logs developer_agent` once this instance stopped
# shipping A2A agents; with none configured there are simply no logs to show.
AGENTS_JSON="$(cli_json admin agents list)"
if [[ "$(printf '%s' "$AGENTS_JSON" | jq '.items | length')" -eq 0 ]]; then
  subheader "STEP 2: Agent Logs"
  echo "  No A2A agents are configured, so no agent processes log here."
  echo "  See the note in services/config/config.yaml. MCP server logs:"
  echo ""
  echo "      systemprompt plugins mcp logs <server-name>"
  echo ""
else
  while IFS= read -r agent; do
    [[ -z "$agent" ]] && continue
    subheader "Agent Logs — $agent"
    run_cli_head 20 admin agents logs "$agent"
  done < <(printf '%s' "$AGENTS_JSON" | jq -r '.items[] | (.agent_id // .id // .name) // empty')
fi

header "AGENT REGISTRY DEMO COMPLETE"
