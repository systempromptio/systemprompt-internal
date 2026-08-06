#!/usr/bin/env bash
# One-shot end-to-end verification of the Linux onboarding flow.
#
#     scripts/clean-client-install.sh sp-live-...
#
# Takes a PAT and proves the whole claim: a machine that has never seen Systemprompt Internal
# ends up running Claude Code against the gateway with the managed MCP servers
# already authenticated. Everything between those two points is automated —
# repackage the tarball, boot a config-free container, run the published
# installer exactly as a user would, then assert the result.
#
# Get the PAT from the running server first (both steps are manual by design —
# the code is a credential and is deliberately not automatable):
#
#     systemprompt admin bridge issue-code --user-id <email|uuid|name>
#     curl -sS -X POST "$GATEWAY/v1/auth/bridge/session-pat" \
#       -H 'content-type: application/json' \
#       -d '{"code":"<CODE>","device_name":"clean-client"}' | jq -r .pat
#
# The code is one-shot and expires in minutes; the PAT it returns is durable.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
IMAGE="systemprompt-clean-client:local"
CONTAINER="systemprompt-clean-client-install"

# The gateway is reached from inside the container, so it cannot be localhost.
GATEWAY="${GATEWAY:-http://host.docker.internal:8080}"
SKIP_PACKAGE="${SKIP_PACKAGE:-0}"

say()  { printf '\033[1m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[33mwarn:\033[0m %s\n' "$*" >&2; }
fail() { printf '\033[31mERROR:\033[0m %s\n' "$*" >&2; exit 1; }

PAT="${1:-${SYSTEMPROMPT_BRIDGE_PAT:-}}"
case "$PAT" in
    sp-live-*) ;;
    "") fail "usage: $0 sp-live-...
       Mint one with 'systemprompt admin bridge issue-code --user-id <you>',
       then POST the code to \$GATEWAY/v1/auth/bridge/session-pat (see header)." ;;
    *)  fail "that does not look like a PAT (expected an sp-live-... value)." ;;
esac

command -v docker >/dev/null 2>&1 || fail "docker is required"

# ── Repackage ─────────────────────────────────────────────────────────────────
# The installer downloads the tarball the gateway serves, so a stale artefact
# silently tests yesterday's binary. package-bridge-linux.sh publishes into
# storage/files/downloads, which the gateway serves at /files/downloads.
if [ "$SKIP_PACKAGE" = "1" ]; then
    warn "SKIP_PACKAGE=1 — testing whatever tarball is already published."
else
    say "building and publishing the Linux bridge tarball"
    ( cd "$REPO_ROOT/bridge" && cargo build --release ) \
        || fail "bridge release build failed"
    "$REPO_ROOT/scripts/package-bridge-linux.sh" \
        || fail "packaging failed"
fi

# ── Gateway reachability ──────────────────────────────────────────────────────
# Checked from the host against the equivalent loopback URL: a failure here is
# almost always "the server is not running", and finding that out before the
# container starts saves a confusing download error inside it.
PROBE="${GATEWAY/host.docker.internal/localhost}"
if ! curl -fsS -o /dev/null --max-time 5 "$PROBE/files/downloads/install.sh"; then
    fail "the gateway is not serving $PROBE/files/downloads/install.sh
       Start it with 'just start' and make sure 'just publish' has run."
fi

# ── Image ─────────────────────────────────────────────────────────────────────
if ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
    say "building the clean-client image"
    docker build -f "$REPO_ROOT/deploy/clean-client/Dockerfile" \
        -t "$IMAGE" "$REPO_ROOT/deploy/clean-client" \
        || fail "image build failed"
fi

docker rm -f "$CONTAINER" >/dev/null 2>&1 || true

# ── Run ───────────────────────────────────────────────────────────────────────
# No repo mount, no bridge bind-mount, no env file: the container must get the
# binary from the gateway exactly as a real user does. The PAT arrives as an
# argument rather than an env var so it is not inherited by anything the
# session later spawns.
say "running the published installer in a config-free container"
docker run --rm --name "$CONTAINER" \
    --hostname clean-client \
    --add-host host.docker.internal:host-gateway \
    -e SYSTEMPROMPT_BRIDGE_GATEWAY_URL="$GATEWAY" \
    --entrypoint bash \
    "$IMAGE" -lc '
set -euo pipefail
GATEWAY="$1"; PAT="$2"

# Stand in for MDM: delegate the enterprise policy directory to the user who
# runs the installer. Without this the bridge cannot write managed-mcp.json and
# correctly degrades to per-plugin provisioning — which is a valid outcome, but
# not the one this run is trying to prove.
sudo install -d -o tester -g tester -m 0755 /etc/claude-code

curl -fsSL "$GATEWAY/files/downloads/install.sh" \
  | sh -s -- --download-base "$GATEWAY/files/downloads" \
             --gateway "$GATEWAY" \
             --pat "$PAT"

echo
echo "── verification ──────────────────────────────────────────────────────"

rc=0

if [ -f /etc/claude-code/managed-mcp.json ]; then
    echo "[OK  ] managed-mcp.json written"
    cat /etc/claude-code/managed-mcp.json
    if grep -q "allowManagedMcpServersOnly" /etc/claude-code/managed-settings.json 2>/dev/null; then
        echo "[OK  ] allowlist locked to managed settings"
    else
        echo "[FAIL] managed-settings.json missing the allowlist"; rc=1
    fi
else
    echo "[FAIL] /etc/claude-code/managed-mcp.json absent — policy not enforced"; rc=1
fi

# `claude mcp list` is the acceptance test the docs describe: it reports what
# the CLI will actually load, after policy evaluation.
echo
echo "claude mcp list:"
claude mcp list || rc=1

# Adding a server must be refused while exclusive control is active. The URL is
# never contacted; the policy check rejects the command first.
echo
if claude mcp add --transport http probe https://example.com/mcp 2>&1 \
     | grep -q "exclusive control"; then
    echo "[OK  ] enterprise policy refuses user-added servers"
else
    echo "[FAIL] a user could still add an MCP server"; rc=1
fi

echo
echo "claude plugin list:"
claude plugin list || true

exit $rc
' _ "$GATEWAY" "$PAT"

say "done"
