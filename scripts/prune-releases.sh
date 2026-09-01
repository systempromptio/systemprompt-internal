#!/usr/bin/env bash
# Retention for the bridge-v* GitHub Releases: keep the N newest (by
# publish date), delete the rest together with their tags. Run by
# .github/workflows/ghcr-prune.yml after every release and weekly; run by
# hand with --dry-run to see what it would do.
#
#   scripts/prune-releases.sh [--keep N] [--dry-run] [--repo owner/name]
set -euo pipefail

KEEP=5
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

# Drafts and prereleases are never counted or deleted: a draft is someone's
# work in progress, a prerelease is a deliberate hold-out.
stale=$(gh release list --repo "$REPO" --limit 200 \
    --json tagName,publishedAt,isDraft,isPrerelease \
    --jq '[.[] | select(.isDraft | not) | select(.isPrerelease | not)
            | select(.tagName | startswith("bridge-v"))]
          | sort_by(.publishedAt) | reverse | .['"$KEEP"':] | .[].tagName')

if [ -z "$stale" ]; then
    echo "nothing to prune: at most $KEEP bridge-v* releases on $REPO"
    exit 0
fi

for tag in $stale; do
    if [ "$DRY_RUN" = 1 ]; then
        echo "would delete release + tag $tag"
    else
        gh release delete "$tag" --repo "$REPO" --yes --cleanup-tag
        echo "deleted release + tag $tag"
    fi
done
