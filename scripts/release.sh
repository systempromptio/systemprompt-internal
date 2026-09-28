#!/usr/bin/env bash
# Promote the exact green next-push candidate onto main through a frozen PR
# (`just release X.Y.Z`; docs/RELEASING.md). First run: verify the candidate,
# push refs/heads/promote/<version>/<main>/<next>, open the PR. Repeat after the
# PR's Gates run finishes: verify that proof and the merge tree, then merge.
# release.yml builds and publishes v<version> and bridge-v<version> from the
# merge commit.
set -euo pipefail
cd "$(dirname "$0")/.."
version="${1:?usage: release.sh X.Y.Z}"
repo=systempromptio/systemprompt-internal
die() { echo "release: $*" >&2; exit 1; }
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die 'expected X.Y.Z'
[ -z "$(git status --porcelain)" ] || die 'working tree is not clean'
branch="$(git branch --show-current)"
[ -z "$branch" ] || [ "$branch" = next ] || die 'use next or a detached release worktree'
! grep -q '^\[patch\.crates-io\]' Cargo.toml tests/Cargo.toml || die 'published releases cannot use local core patches'
# Lockstep: the release version is the core version, so this one check covers
# the workspace, the bridge, every core pin and bridge/CORE_REF.
scripts/sync-release-version.sh "$version" --check
bash scripts/check-release-version.sh
bash scripts/check-core-ref.sh
bash scripts/check-schema-baseline.sh
git fetch origin main next
sha="$(git rev-parse HEAD)"
[ "$sha" = "$(git rev-parse origin/next)" ] || die 'HEAD differs from origin/next'
base="$(git rev-parse origin/main)"
[ "$sha" != "$base" ] || die 'candidate is already main'
git merge-base --is-ancestor "$base" "$sha" || die 'main is not an ancestor of candidate'
bash scripts/check-gates-green.sh "$sha"
ref="promote/$version/$base/$sha"
remote="$(git ls-remote origin "refs/heads/$ref" | cut -f1)"
if [ -z "$remote" ]; then
    git push origin "$sha:refs/heads/$ref"
else
    [ "$remote" = "$sha" ] || die 'promotion ref differs from frozen candidate'
fi
pr="$(gh pr list -R "$repo" --base main --head "$ref" --state open --json number --jq '.[0].number // empty')"
if [ -z "$pr" ]; then
    body="$(mktemp)"
    trap 'rm -f "$body"' EXIT
    printf 'Release %s against published core crates.\n\nFrozen candidate: `%s`\nMain base: `%s`\n\nThe exact next-push Gates run proves the candidate; promotion verifies that proof and the merge tree without repeating the matrix.\n' "$version" "$sha" "$base" > "$body"
    gh pr create -R "$repo" --base main --head "$ref" --title "Release $version" --body-file "$body"
    pr="$(gh pr list -R "$repo" --base main --head "$ref" --state open --json number --jq '.[0].number')"
fi
state="$(gh pr view "$pr" -R "$repo" --json headRefOid,baseRefOid,mergeable)"
jq -e --arg sha "$sha" --arg base "$base" '.headRefOid == $sha and .baseRefOid == $base' <<<"$state" >/dev/null || die 'PR head/base moved'
if proof="$(bash scripts/check-promotion-green.sh "$sha" "$ref")"; then
    IFS=$'\t' read -r run_id attempt <<<"$proof"
    echo "Promotion PR #$pr verified by run $run_id attempt $attempt"
else
    status=$?
    if [ "$status" -ne 2 ]; then die 'promotion proof failed; inspect the exact PR run'; fi
    echo "Promotion PR #$pr is open; record its run ID, wait for its proof, then repeat: just release $version"
    exit 0
fi
[ "$(jq -r .mergeable <<<"$state")" = MERGEABLE ] || die 'PR is not mergeable'
git fetch origin main
git fetch origin "refs/pull/$pr/merge"
[ "$(git rev-parse origin/main)" = "$base" ] || die 'main moved after proof'
[ "$(git rev-parse FETCH_HEAD^{tree})" = "$(git rev-parse "$sha^{tree}")" ] || die 'proposed merge tree differs from proven candidate'
bash scripts/check-gates-green.sh "$sha"
gh pr merge "$pr" -R "$repo" --merge --match-head-commit "$sha"
git fetch origin main
merge="$(git rev-parse origin/main)"
[ "$(git rev-parse "$merge^1")" = "$base" ] || die 'merged main base changed'
[ "$(git rev-parse "$merge^2")" = "$sha" ] || die 'merged candidate changed'
[ "$(git rev-parse "$merge^{tree}")" = "$(git rev-parse "$sha^{tree}")" ] || die 'merged tree differs from proven candidate'
echo "main -> $merge through PR #$pr; watch release.yml for v$version and bridge-v$version"
