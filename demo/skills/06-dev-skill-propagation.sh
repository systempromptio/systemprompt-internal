#!/bin/bash
# DEMO: CENTRAL SKILL UPDATE → BRIDGE PROPAGATION
# The POC's core promise: a skill edited once at the gateway reaches every
# developer assigned to the profile, with nothing to pull or reconfigure.
#
# What this does:
#   1. Fetches the astound-dev plugin manifest over the bridge plugin endpoint
#   2. Stamps a unique marker into services/skills/dev-plan/SKILL.md
#   3. Re-runs the publish pipeline (the same job that runs at server startup)
#   4. Fetches the served dev_plan skill body and asserts the marker appears
#   5. Restores the skill file and republishes
#
# Cost: Free (no AI call)

set -e

source "$(cd "$(dirname "$0")/.." && pwd)/_common.sh"
load_token

header "SKILLS: CENTRAL UPDATE PROPAGATION" "Edit once at the gateway, every developer gets it"

SKILL_FILE="$PROJECT_DIR/services/skills/dev_plan/SKILL.md"
PLUGIN_URL="$BASE_URL/v1/bridge/plugins/astound-dev"
MARKER="propagation-check-$$-$(date +%s)"

# The bridge plugin endpoint takes a first-party (web/api/a2a/mcp) JWT; the
# hook-audience token in demo/.token is rejected there. Mint a session token.
BRIDGE_TOKEN=$("$CLI" admin session login --token-only --profile "$PROFILE" 2>/dev/null | tail -1)
if [[ -z "$BRIDGE_TOKEN" ]]; then
  echo "  ERROR: could not mint an admin session token"
  exit 1
fi

if [[ ! -f "$SKILL_FILE" ]]; then
  echo "  ERROR: $SKILL_FILE not found — is the astound-dev plugin present?"
  exit 1
fi

restore() {
  if [[ -f "$SKILL_FILE.bak" ]]; then
    mv "$SKILL_FILE.bak" "$SKILL_FILE"
    run_cli infra jobs run publish_pipeline >/dev/null 2>&1 || true
  fi
}
trap restore EXIT

subheader "STEP 1: Baseline — served skill body has no marker"
BEFORE=$(curl -sf -H "Authorization: Bearer $BRIDGE_TOKEN" "$PLUGIN_URL/skills/dev-plan/SKILL.md" || true)
if [[ -z "$BEFORE" ]]; then
  echo "  ERROR: could not fetch $PLUGIN_URL/skills/dev-plan/SKILL.md"
  echo "  Is the server running and the token valid? (demo/00-preflight.sh)"
  exit 1
fi
if grep -q "$MARKER" <<<"$BEFORE"; then
  echo "  ERROR: marker already present before the edit — aborting"
  exit 1
fi
echo "  Served skill body fetched ($(wc -c <<<"$BEFORE") bytes), marker absent. Good."

subheader "STEP 2: Edit the skill centrally"
cp "$SKILL_FILE" "$SKILL_FILE.bak"
printf '\n<!-- %s -->\n' "$MARKER" >> "$SKILL_FILE"
echo "  Appended marker '$MARKER' to services/skills/dev-plan/SKILL.md"

subheader "STEP 3: Re-run the publish pipeline"
run_cli infra jobs run publish_pipeline

subheader "STEP 4: Fetch the served skill again — marker must appear"
AFTER=$(curl -sf -H "Authorization: Bearer $BRIDGE_TOKEN" "$PLUGIN_URL/skills/dev-plan/SKILL.md")
if grep -q "$MARKER" <<<"$AFTER"; then
  echo "  ✔ Marker found in the served bundle. Central edit propagated."
else
  echo "  ✘ FAILED: marker not present in the served skill body."
  exit 1
fi

subheader "STEP 5: Restore the original skill"
restore
trap - EXIT
echo "  services/skills/dev-plan/SKILL.md restored and republished."

header "PROPAGATION DEMO COMPLETE" "Edit → publish → served to every bridge, no client action"
