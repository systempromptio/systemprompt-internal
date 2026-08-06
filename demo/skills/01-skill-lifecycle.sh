#!/bin/bash
# DEMO: SKILL LIFECYCLE — LIST, DISK CONFIG
# Read-only skill management operations.
#
# What this does:
#   1. Lists all database-synced skills with their on-disk config paths
#   2. Walks the nested skill directories on disk
#   3. Shows one skill in full (config + markdown body)
#
# Cost: Free (no AI call)

set -e

source "$(cd "$(dirname "$0")/.." && pwd)/_common.sh"

header "SKILLS: LIFECYCLE" "List, disk configuration"

subheader "STEP 1: List Database-Synced Skills"
run_cli_head 30 core skills list

subheader "STEP 2: Nested skill directories on disk"
SKILLS_DIR="$PROJECT_DIR/services/skills"
echo "  \$ ls $SKILLS_DIR/*/config.yaml"
echo ""
for cfg in "$SKILLS_DIR"/*/config.yaml; do
  [[ -f "$cfg" ]] || continue
  echo "$cfg" | sed "s|$PROJECT_DIR/||" | sed 's/^/    /'
done
echo ""

# Show whichever skill the registry reports first rather than a hard-coded id.
# The old `use_dangerous_secret` example skill no longer ships — the catalogue
# was rebuilt Salesforce-first — and naming any single skill here just means
# this demo breaks again the next time the catalogue changes.
subheader "STEP 3: Show one skill in full (config + instructions)"
# The list rows key the skill as `skill_id`; `name` is the display title
# ("Find Accounts"), which `skills show` does not resolve.
SKILL_ID="$(cli_json core skills list | jq -r '.items[0] | (.skill_id // .id) // empty')"
assert_nonempty "$SKILL_ID" "a skill is registered to show"
echo "  showing: $SKILL_ID"
echo ""
run_cli_head 40 core skills show "$SKILL_ID"

header "SKILL LIFECYCLE DEMO COMPLETE" "Showed: list, nested config layout"
