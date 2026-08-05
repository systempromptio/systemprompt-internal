#!/usr/bin/env bash
# Entrypoint for the clean-client test container.
#
# Two jobs: fail loudly if the container was started with host config leaking
# into it, and print the exact sequence to run. Then get out of the way.
set -euo pipefail

fail() { printf '\033[31mERROR:\033[0m %s\n' "$1" >&2; exit 1; }
warn() { printf '\033[33mwarn:\033[0m %s\n' "$1" >&2; }

# ── Cleanliness assertions ────────────────────────────────────────────────────
# A single convenience mount of the host's ~/.claude defeats the entire purpose
# of this container, and the failure mode is silent: the test passes because it
# is reading config you already had. So refuse to start.
for leaked in \
    "$HOME/.claude.json" \
    "$HOME/.claude/settings.json" \
    "$HOME/.config/astound/astound-bridge.toml"
do
    if [ -e "$leaked" ] && [ "${CLEAN_CLIENT_ALLOW_STATE:-0}" != "1" ]; then
        fail "$leaked already exists — this container is not clean.
       Either it was started with a host bind-mount, or you are reusing the
       state volume from a previous run. Run 'just clean-client-reset' to wipe
       it, or set CLEAN_CLIENT_ALLOW_STATE=1 to keep a login across runs."
    fi
done

if [ -n "${ANTHROPIC_API_KEY:-}" ]; then
    warn "ANTHROPIC_API_KEY is set in this container. Claude Code will use it
      directly and bypass the gateway, so the integration will not be exercised.
      Unset it unless that is what you are testing."
fi

# ── Bridge availability ───────────────────────────────────────────────────────
if command -v astound-bridge >/dev/null 2>&1; then
    BRIDGE_STATUS="$(astound-bridge --version 2>/dev/null || echo 'present (version unavailable)')"
else
    BRIDGE_STATUS="NOT MOUNTED — build it with: cd bridge && cargo build --release"
fi

cat <<BANNER

  Astound clean client — no config, no host state.

  gateway   ${ASTOUND_BRIDGE_GATEWAY_URL:-<unset>}
  bridge    ${BRIDGE_STATUS}
  home      ${HOME}

  Integration test sequence (device cert = the supported headless path):

    mkdir -p ~/.config/astound
    openssl req -x509 -newkey rsa:2048 -nodes -days 730 \
      -keyout ~/.config/astound/device.key \
      -out    ~/.config/astound/device.pem -subj "/CN=$(hostname)"
    openssl x509 -in ~/.config/astound/device.pem -outform der | sha256sum

    # admin enrols that fingerprint:
    #   systemprompt admin bridge enroll-cert --user-id <uuid> --fingerprint <hex>

    printf '\n[mtls]\ncert_keystore_ref = "%s/.config/astound/device.pem"\n' "$HOME" \
      >> ~/.config/astound/astound-bridge.toml

    astound-bridge whoami                  # your identity, no PAT needed
    astound-bridge install --apply --apply-schedule
    astound-bridge sync --allow-tofu       # plugins, skills, agents, MCP
    astound-bridge proxy &                 # no systemd here, so start it by hand
    bash -l -c claude                      # org skills load, no manual exports

  'install --apply' writes ~/.config/astound/env.sh (ANTHROPIC_BASE_URL and
  ANTHROPIC_AUTH_TOKEN) plus a managed block in ~/.profile that sources it, so a
  login shell needs no exports. '--apply-schedule' writes the systemd user units
  for sync and the proxy; this container has no systemd, so it warns and you run
  'astound-bridge proxy' yourself. That degradation is part of what this
  container tests.

  Run 'astound-bridge doctor' if anything looks wrong — it names the cause,
  including whether the proxy is listening. With no Secret Service provider the
  bridge tiers down to the kernel keyring, then to process memory, and says so.

BANNER

# ── Project mount (dev-sandbox flow) ─────────────────────────────────────────
# `just dev-sandbox <repo>` mounts a project at /workspace/project. Start the
# session there so Claude Code opens on the project, not the empty work dir.
# Only the project directory is mounted — HOME stays virgin, so all the
# cleanliness assertions above still hold.
if [ -d /workspace/project ]; then
    echo "  project   /workspace/project (mounted — session starts here)"
    echo
    cd /workspace/project
fi

exec "$@"
