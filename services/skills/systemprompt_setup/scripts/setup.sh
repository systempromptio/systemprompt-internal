#!/bin/sh
# One-shot staging for the Cowork setup skills, run inside the Cowork session VM.
# Shared by systemprompt_setup (workspace dashboards) and
# systemprompt_setup_admin (control-plane dashboards); it ships once, here.
#
#   setup.sh              find every plugin bundle the bridge mounted, copy each
#                         bundled dashboard into the session outputs dir, and
#                         print one create_artifact parameter block per artifact
#   setup.sh -- <plugin>  the same, restricted to bundles whose directory name
#                         contains <plugin>; prefix with '!' to exclude instead.
#                         Used by the role-split setup skills so the user setup
#                         ('!systemprompt-admin') and the admin setup
#                         ('systemprompt-admin') each stage only their own
#                         dashboards rather than every mounted bundle.
#   setup.sh receipt '<json>'
#                         write the given receipt JSON to outputs/setup-receipt.json,
#                         replacing the literal token __NOW__ with the current
#                         ISO-8601 UTC timestamp
#
# The mount is still the role grant: the bridge mounts exactly the plugins the
# signed manifest granted this user, so a bundle the caller was not granted is
# never present to be filtered in the first place. The filter only decides which
# of the caller's own bundles a given setup skill installs. Every bundle lays
# its dashboards out as artifacts/manifest.json (install records) plus one
# artifacts/<id>.html per record (the page, verbatim).
#
# POSIX sh. Prints OUTPUTS_DIR= on success so the caller never has to guess the
# mount layout.
set -eu

RECEIPT_FILE="setup-receipt.json"

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

find_manifests() {
    all=$(find "$HOME/mnt" /sessions/*/mnt -maxdepth 6 -type f -name manifest.json -path '*/artifacts/manifest.json' 2>/dev/null | sort -u)
    [ -n "$PLUGIN_FILTER" ] || { printf '%s\n' "$all"; return 0; }
    case "$PLUGIN_FILTER" in
        !*) want=0; pat=${PLUGIN_FILTER#!} ;;
        *)  want=1; pat=$PLUGIN_FILTER ;;
    esac
    printf '%s\n' "$all" | while IFS= read -r m; do
        [ -n "$m" ] || continue
        hit=0
        case "$(basename -- "$(dirname -- "$(dirname -- "$m")")")" in
            *"$pat"*) hit=1 ;;
        esac
        if [ "$hit" = "$want" ]; then
            printf '%s\n' "$m"
        fi
    done
    return 0
}

PLUGIN_FILTER=""
if [ "${1:-}" = "--" ]; then
    [ $# -ge 2 ] || { echo "usage: setup.sh -- <plugin-id>" >&2; exit 2; }
    PLUGIN_FILTER="$2"
    shift 2
fi

OUTPUTS=$(find_outputs)

if [ "${1:-}" = "receipt" ]; then
    [ $# -ge 2 ] || { echo "usage: setup.sh receipt '<json>'" >&2; exit 2; }
    now=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    printf '%s\n' "$2" | sed "s/__NOW__/$now/" > "$OUTPUTS/$RECEIPT_FILE"
    echo "RECEIPT_WRITTEN=$OUTPUTS/$RECEIPT_FILE"
    cat "$OUTPUTS/$RECEIPT_FILE"
    exit 0
fi

MANIFESTS=$(find_manifests)
if [ -z "$MANIFESTS" ]; then
    if [ -n "$PLUGIN_FILTER" ]; then
        echo "ERROR: no mounted bundle selected by '$PLUGIN_FILTER' carries artifacts/manifest.json — either it was not granted to this user or bridge sync has not mounted it" >&2
    else
        echo "ERROR: no plugin bundle under the mounts carries artifacts/manifest.json — bridge sync has not mounted any dashboards" >&2
    fi
    exit 1
fi

count=0
plugins=""
for manifest in $MANIFESTS; do
    dir=$(dirname -- "$manifest")
    plugin=$(basename -- "$(dirname -- "$dir")")
    plugins="$plugins $plugin"
    for f in "$dir"/*.html; do
        [ -e "$f" ] || continue
        cp "$f" "$OUTPUTS/"
        count=$((count + 1))
    done
done
[ "$count" -gt 0 ] || { echo "ERROR: the mounted bundles list dashboards but ship no HTML beside them" >&2; exit 1; }

echo "OUTPUTS_DIR=$OUTPUTS"
echo "PLUGINS=$(printf '%s' "$plugins" | sed 's/^ //')"
echo "COPIED=$count"
echo
echo "== create_artifact parameter blocks (one call per block, sequential; deduped by id) =="
if command -v python3 >/dev/null 2>&1; then
    # shellcheck disable=SC2086 # manifest paths are newline-separated, no globs
    python3 - "$OUTPUTS" $MANIFESTS <<'PY'
import json, sys
outputs = sys.argv[1]
seen = set()
for manifest in sys.argv[2:]:
    with open(manifest, encoding="utf-8") as fh:
        records = json.load(fh)["artifacts"]
    for a in records:
        if a["id"] in seen:
            continue
        seen.add(a["id"])
        block = {
            "id": a["id"],
            "description": a["description"],
            "html_path": f"{outputs}/{a['id']}.html",
            "mcp_tools": a["mcpTools"],
        }
        print(json.dumps(block, ensure_ascii=False))
        extras = {"name": a["name"], "starred": a["isStarred"], "version": a["version"]}
        print(f"# also pass, if the tool schema exposes such fields: {json.dumps(extras, ensure_ascii=False)}")
        print()
print(f"TOTAL_RECORDS={len(seen)}")
PY
else
    echo "(python3 unavailable — read each manifest below and build the blocks yourself:"
    echo " html_path is $OUTPUTS/<id>.html; pass id, description, mcp_tools, and name/star if supported)"
    echo
    for manifest in $MANIFESTS; do
        echo "== $manifest =="
        cat "$manifest"
    done
fi
