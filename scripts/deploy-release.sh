#!/usr/bin/env bash
# Deploy a published release to production from a clean worktree of
# origin/main. Invoked by `just deploy-release <version>`.
#
# Why a worktree: `just deploy` ships whatever the working tree holds. In
# 0.52.0 a peer had re-activated [patch.crates-io] on unreleased core in the
# shared checkout; in 0.53.0 the local `main` branch was stale and a peer held
# 44 modified files. The release is a commit on origin/main and nothing else.
#
# Why the release assets: the same `cargo build --release --workspace` that
# built the image built the server tarball. Building here would ship a
# different binary from the one the release signed. The desktop bridge is not
# baked into this image at all — the admin pages link to the bridge-v<version>
# GitHub release — so there are no downloads to stage.
#
# Linux only: the tarball's binaries are baked into a linux/amd64 image AND
# run here for publish_pipeline and `cloud doctor`, so the host must be able
# to execute them.
#
# Why the five-minute watch: 0.53.0's machine answered healthy at +1 min and
# 503 from +2 to +5 min while the shared database stalled under the new
# instance's first scheduler runs. One probe is not a deploy verification.
set -euo pipefail

VERSION="${1:?usage: deploy-release.sh <version>}"
case "$VERSION" in v*) VERSION="${VERSION#v}" ;; esac
TAG="v$VERSION"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKTREE="$(dirname "$ROOT")/$(basename "$ROOT")-deploy-$VERSION"
LOCAL_PROFILE=".systemprompt/profiles/local/profile.yaml"
PROD_PROFILE=".systemprompt/profiles/production/profile.yaml"
# DEPLOY_DRY_RUN=1 stops after the preflight, before `cloud deploy`.
WATCH_SECONDS="${DEPLOY_WATCH_SECONDS:-300}"
WATCH_INTERVAL=15
DIGEST_WAIT_SECONDS="${DEPLOY_DIGEST_WAIT_SECONDS:-120}"
DIGEST_WAIT_INTERVAL=5
HEALTH_READY_SECONDS="${DEPLOY_HEALTH_READY_SECONDS:-300}"
REPO=systempromptio/systemprompt-internal
IMAGE=ghcr.io/systempromptio/systemprompt-internal

# A desktop/credential docker shim fails `docker build` with an error naming
# neither; the real binary is /usr/bin/docker.
if [ -x /usr/bin/docker ]; then export PATH=/usr/bin:$PATH; fi

log() { printf '\n==> %s\n' "$*"; }
die() { echo "deploy-release: $*" >&2; exit 1; }
[[ "$WATCH_SECONDS" =~ ^[0-9]+$ ]] && [ "$WATCH_SECONDS" -ge 300 ] || die 'health watch must last at least 300 seconds'
source "$ROOT/scripts/deploy-observation.sh"

[ "$(uname -s)" = Linux ] || die "the release tarball is Linux-only; deploy a release from a Linux host (or 'just deploy' from source)"
[ "$(uname -m)" = x86_64 ] || die "the production image is linux/amd64; run deploy-release on an x86_64 host"
for tool in git gh flyctl jq curl docker just; do
    command -v "$tool" >/dev/null 2>&1 || die "$tool is required"
done
gh auth status >/dev/null 2>&1 || die "gh is not authenticated (the repository is private)"
[ -f "$ROOT/$PROD_PROFILE" ] || die "no production profile at $ROOT/$PROD_PROFILE"

tenant_id="$(sed -n 's/^  tenant_id: *//p' "$ROOT/$PROD_PROFILE" | head -1)"
[ -n "$tenant_id" ] || die "production profile has no tenant_id"
FLY_APP="sp-$(printf '%s' "$tenant_id" | tr -d '-' | cut -c1-12)"
HOST="$(sed -n 's/^  api_external_url: *//p' "$ROOT/$PROD_PROFILE" | head -1)"
[ -n "$HOST" ] || die "production profile has no api_external_url"

log "release $TAG must be origin/main"
git -C "$ROOT" fetch -q origin main "refs/tags/$TAG:refs/tags/$TAG" 2>/dev/null \
    || git -C "$ROOT" fetch -q origin main
main_sha="$(git -C "$ROOT" rev-parse origin/main)"
tag_sha="$(git -C "$ROOT" rev-parse "$TAG^{commit}" 2>/dev/null || true)"
[ -n "$tag_sha" ] || die "tag $TAG does not exist — release.yml has not published $VERSION yet"
[ "$tag_sha" = "$main_sha" ] || die "tag $TAG is $tag_sha but origin/main is $main_sha — deploy the release, not a later main"
gh release view "$TAG" -R "$REPO" --json assets --jq '.assets|length' >/dev/null \
    || die "no GitHub release $TAG"
gh release view "bridge-v$VERSION" -R "$REPO" --json tagName >/dev/null \
    || die "no GitHub release bridge-v$VERSION — the admin pages would link to a missing bridge"
docker manifest inspect "$IMAGE:$VERSION" >/dev/null 2>&1 \
    || die "GHCR has no :$VERSION image — release.yml did not finish"
echo "    origin/main = $TAG = ${main_sha:0:12}"

log "worktree $WORKTREE"
[ ! -e "$WORKTREE" ] || die "$WORKTREE exists — a previous deploy left it for inspection; remove it first"
git -C "$ROOT" worktree add --detach "$WORKTREE" "$main_sha" >/dev/null
cd "$WORKTREE"
[ -z "$(git status --porcelain)" ] || die "fresh worktree is not clean"
grep -q '^\[patch\.crates-io\]' Cargo.toml tests/Cargo.toml && die "[patch.crates-io] is active on main"
cp -r "$ROOT/.systemprompt" .
[ ! -f "$ROOT/signing_key.pem" ] || cp "$ROOT/signing_key.pem" .
# The local profile carries absolute paths; publish_pipeline renders web/dist
# at `paths.system`, which must be THIS tree or the image COPYs the old one.
sed -i -E \
    -e "s#^(  system:) .*#\\1 $WORKTREE#" \
    -e "s#^(  services:) .*#\\1 $WORKTREE/services#" \
    -e "s#^(  bin:) .*#\\1 $WORKTREE/target/release#" \
    -e "s#^(  storage:) .*#\\1 $WORKTREE/storage#" \
    "$LOCAL_PROFILE"
grep -q "^  system: $WORKTREE\$" "$LOCAL_PROFILE" || die "could not re-point the local profile at the worktree"
grep -q "^  bin: $WORKTREE/target/release\$" "$LOCAL_PROFILE" || die "could not re-point paths.bin at the release binaries"

CORE_VERSION="$(sed -n 's/^systemprompt = { version = "\([0-9.]*\)".*/\1/p' Cargo.toml | head -1)"
[ -n "$CORE_VERSION" ] || die "could not read the core pin from Cargo.toml"

log "binaries from release $TAG"
just fetch-release "$VERSION"

log "web/dist"
SYSTEMPROMPT_PROFILE="$WORKTREE/$LOCAL_PROFILE" target/release/systemprompt infra jobs run publish_pipeline --profile local
[ -f web/dist/index.html ] || die "publish_pipeline did not render web/dist here"

log "preflight"
target/release/systemprompt cloud doctor --profile production --distributed

before_state="$(machine_state)"
before="${before_state%% *}"
echo "    $FLY_APP image before: $before (updated ${before_state#* })"

if [ "${DEPLOY_DRY_RUN:-0}" = "1" ]; then
    cd "$ROOT"
    git worktree remove --force "$WORKTREE"
    log "DEPLOY_DRY_RUN=1: everything up to 'cloud deploy' passed for $TAG; nothing was deployed"
    exit 0
fi

log "cloud deploy --profile production"
target/release/systemprompt cloud deploy --profile production

observe_deploy

cd "$ROOT"
git worktree remove --force "$WORKTREE"

log "deployed $TAG to $FLY_APP"
echo "    main/tag: ${main_sha:0:12}"
echo "    image:    $before -> $after"
echo "    health:   core $CORE_VERSION steady for ${WATCH_SECONDS}s"
echo "Record these in the release notes; a version string alone is not deploy evidence."
