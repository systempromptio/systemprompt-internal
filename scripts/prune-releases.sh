#!/usr/bin/env bash
# Retention for the GitHub Releases this repo publishes. Every core release
# yields exactly two: `v<X.Y.Z>` (gateway tarballs) and `bridge-v<X.Y.Z>`
# (desktop bridge). Keep the N newest of each series (by publish date) and
# delete the rest together with their tags; also drop a `bridge-v*` tag that
# has no release behind it (a release deleted by hand leaves one).
#
#   scripts/prune-releases.sh [--keep N] [--dry-run] [--repo owner/name]
#
# Drafts and prereleases are never counted or deleted: a draft is someone's
# work in progress, a prerelease is a deliberate hold-out.
set -euo pipefail

KEEP=3
DRY_RUN=0
REPO="${GITHUB_REPOSITORY:-systempromptio/systemprompt-internal}"
while [ $# -gt 0 ]; do
    case "$1" in
        --keep)    KEEP="$2"; shift 2 ;;
        --dry-run) DRY_RUN=1; shift ;;
        --repo)    REPO="$2"; shift 2 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

command -v gh >/dev/null || { echo "gh is required" >&2; exit 1; }

releases=$(gh release list --repo "$REPO" --limit 200 \
    --json tagName,publishedAt,isDraft,isPrerelease \
    --jq '[.[] | select(.isDraft | not) | select(.isPrerelease | not)]')

act() { # $1=verb $2=what
    if [ "$DRY_RUN" = 1 ]; then echo "would $1 $2"; else echo "$1 $2"; fi
}

deleted=0
for prefix in "bridge-v" "v"; do
    # `v` must not swallow `bridge-v`: match the prefix at the start only.
    stale=$(printf '%s' "$releases" | jq -r --arg p "$prefix" --argjson keep "$KEEP" '
        [.[] | select(.tagName | startswith($p))
             | select(($p != "v") or ((.tagName | startswith("bridge-")) | not))]
        | sort_by(.publishedAt) | reverse | .[$keep:] | .[].tagName')
    for tag in $stale; do
        act "delete release + tag" "$tag"
        [ "$DRY_RUN" = 1 ] || gh release delete "$tag" --repo "$REPO" --yes --cleanup-tag
        deleted=$((deleted + 1))
    done
done

# Orphan bridge tags: a tag with no release is not a download anyone can find.
with_release=$(printf '%s' "$releases" | jq -r '.[].tagName')
for tag in $(gh api "repos/$REPO/git/matching-refs/tags/bridge-v" --jq '.[].ref' | sed 's|refs/tags/||'); do
    if ! printf '%s\n' "$with_release" | grep -qx "$tag"; then
        act "delete orphan tag" "$tag"
        [ "$DRY_RUN" = 1 ] || gh api -X DELETE "repos/$REPO/git/refs/tags/$tag" >/dev/null
        deleted=$((deleted + 1))
    fi
done

[ "$deleted" -gt 0 ] || echo "nothing to prune: at most $KEEP releases per series on $REPO"
