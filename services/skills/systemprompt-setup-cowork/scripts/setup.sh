#!/bin/sh
# One-shot staging for the systemprompt-setup-cowork skill, run inside the Cowork session VM.
#
#   setup.sh              copy every bundled dashboard into the session outputs
#                         dir and print one create_artifact parameter block per
#                         artifact, ready to mirror into tool calls
#   setup.sh receipt '<json>'
#                         write the given receipt JSON to outputs/setup-receipt.json,
#                         replacing the literal token __NOW__ with the current
#                         ISO-8601 UTC timestamp
#
# POSIX sh; no arguments beyond the mode. Prints OUTPUTS_DIR= on success so the
# caller never has to guess the mount layout.
set -eu

RECEIPT_FILE="setup-receipt.json"

SKILL_DIR=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
ASSETS="$SKILL_DIR/assets/artifacts"

find_outputs() {
    for candidate in "$HOME/mnt/outputs" /sessions/*/mnt/outputs; do
        if [ -d "$candidate" ] && [ -w "$candidate" ]; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done
    echo "ERROR: no writable outputs directory found under \$HOME/mnt or /sessions/*/mnt" >&2
    echo "NOTE: bash sees only Linux VM paths — do not retry with C:\\ paths; discover with find." >&2
    return 1
}

OUTPUTS=$(find_outputs)

if [ "${1:-}" = "receipt" ]; then
    [ $# -ge 2 ] || { echo "usage: setup.sh receipt '<json>'" >&2; exit 2; }
    now=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    printf '%s\n' "$2" | sed "s/__NOW__/$now/" > "$OUTPUTS/$RECEIPT_FILE"
    echo "RECEIPT_WRITTEN=$OUTPUTS/$RECEIPT_FILE"
    cat "$OUTPUTS/$RECEIPT_FILE"
    exit 0
fi

[ -d "$ASSETS" ] || { echo "ERROR: $ASSETS missing — bridge sync has not staged the dashboards" >&2; exit 1; }

count=0
for f in "$ASSETS"/*.html; do
    [ -e "$f" ] || break
    cp "$f" "$OUTPUTS/"
    count=$((count + 1))
done
[ "$count" -gt 0 ] || { echo "ERROR: no dashboard HTML files in $ASSETS" >&2; exit 1; }

echo "OUTPUTS_DIR=$OUTPUTS"
echo "COPIED=$count"
echo
echo "== create_artifact parameter blocks (one call per block, sequential) =="
if command -v python3 >/dev/null 2>&1; then
    python3 - "$ASSETS/manifest.json" "$OUTPUTS" <<'PY'
import json, sys
manifest, outputs = sys.argv[1], sys.argv[2]
for a in json.load(open(manifest))["artifacts"]:
    block = {
        "id": a["id"],
        "description": a["description"],
        "html_path": f"{outputs}/{a['id']}.html",
        "mcp_tools": a["mcpTools"],
    }
    print(json.dumps(block, ensure_ascii=False))
    extras = {"name": a["name"], "starred": a["isStarred"]}
    print(f"# also pass, if the tool schema exposes such fields: {json.dumps(extras, ensure_ascii=False)}")
    print()
PY
else
    echo "(python3 unavailable — read the manifest below and build the blocks yourself:"
    echo " html_path is $OUTPUTS/<id>.html; pass id, description, mcp_tools, and name/star if supported)"
    echo
    cat "$ASSETS/manifest.json"
fi
