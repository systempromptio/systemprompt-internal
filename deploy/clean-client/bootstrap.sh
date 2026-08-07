#!/usr/bin/env bash
# Brings a clean client all the way to a working `claude`, asking for exactly
# one thing: the device-link code.
#
# Everything else — install, proxy, sync — is derivable, so it runs unattended.
# The code is not: it is a credential, and proving identity through a browser is
# the point of the flow rather than an obstacle to it.
set -euo pipefail

GATEWAY="${SYSTEMPROMPT_BRIDGE_GATEWAY_URL:?gateway URL not set}"

# The container reaches the gateway at host.docker.internal; a browser on the
# host reaches the same server at localhost. Printing the container's view would
# send you to a hostname your browser cannot resolve.
BROWSER_GATEWAY="${GATEWAY//host.docker.internal/localhost}"

bold() { printf '\033[1m%s\033[0m\n' "$*"; }
step() { printf '\n\033[1m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[33mwarn:\033[0m %s\n' "$*" >&2; }

if ! command -v systemprompt-internal-bridge >/dev/null 2>&1; then
    printf '\033[31mERROR:\033[0m systemprompt-internal-bridge is not mounted. Run `just bridge-build` first.\n' >&2
    exit 1
fi

# ── 1. Credentials ────────────────────────────────────────────────────────────
# Skip the prompt when the state volume already carries a PAT that still works;
# re-authenticating a working client would just burn a code.
if [ -f "$HOME/.config/systemprompt-internal/systemprompt-internal-bridge.pat" ] && systemprompt-internal-bridge whoami >/dev/null 2>&1; then
    step "already signed in — reusing the stored PAT"
elif [ -n "${SYSTEMPROMPT_BRIDGE_CODE:-}" ]; then
    # The caller already has the code (it came off the profile page as an
    # argument), so there is nothing to ask for.
    step "signing in with the supplied code"
    systemprompt-internal-bridge login --code "$SYSTEMPROMPT_BRIDGE_CODE" \
        --gateway "$GATEWAY" --device-name clean-client
else
    step "sign in"
    bold "  Open this in your browser:"
    printf '\n      %s/bridge-auth/device-link\n\n' "$BROWSER_GATEWAY"
    echo "  Sign in with Odoo, click Allow. The page THEN shows a one-time code"
    echo "  (the 'Just the code' section) — that code is what goes here, not"
    echo "  your username or email."
    echo

    # A mispaste must not abort the whole bootstrap — a fresh code is a browser
    # round-trip away, so ask again rather than dying under set -e.
    SIGNED_IN=0
    for _ in 1 2 3; do
        printf '  code: '
        read -r CODE

        # Terminals with bracketed paste wrap a paste in ESC[200~ … ESC[201~.
        # Readline strips those at an interactive prompt; `read` in a script
        # does not, so without this the code arrives with an invisible prefix
        # and the gateway rejects a string the screen shows as correct.
        CODE=$(printf '%s' "$CODE" | sed $'s/\033\\[[0-9;]*[~A-Za-z]//g' | tr -d '\000-\037')
        # Then the surrounding whitespace a paste often carries. `read` strips
        # its own, but stripping an escape can expose more, so trim after
        # rather than relying on what came before.
        CODE="${CODE#"${CODE%%[![:space:]]*}"}"
        CODE="${CODE%"${CODE##*[![:space:]]}"}"

        case "$CODE" in
        *@*)
            warn "that looks like an email address — paste the code the page shows after you click Allow"
            continue
            ;;
        '')
            warn "nothing pasted — copy the code from the device-link page"
            continue
            ;;
        esac

        # Tolerant on purpose: the page shows a command as well as a bare code,
        # and the CLI accepts either. Refusing a paste here would be gratuitous.
        if systemprompt-internal-bridge login --code "$CODE" --gateway "$GATEWAY" --device-name clean-client; then
            SIGNED_IN=1
            break
        fi
        warn "sign-in failed — codes are one-shot and short-lived, so refresh the page for a fresh one and try again"
    done
    if [ "$SIGNED_IN" -ne 1 ]; then
        printf '\033[31mERROR:\033[0m could not sign in after 3 attempts.\n' >&2
        exit 1
    fi
fi

# ── 2. Policy + plugins ───────────────────────────────────────────────────────
step "installing integration"
systemprompt-internal-bridge install --apply --apply-schedule

step "starting the loopback proxy"
# No systemd in this container, which is why --apply-schedule warned above.
if systemprompt-internal-bridge doctor 2>/dev/null | grep -q '^\[OK  \] inference proxy'; then
    echo "    already running"
else
    systemprompt-internal-bridge proxy >"$HOME/.cache/bridge-proxy.log" 2>&1 &
    for _ in $(seq 1 20); do
        systemprompt-internal-bridge doctor 2>/dev/null | grep -q '^\[OK  \] inference proxy' && break
        sleep 0.5
    done
fi

step "syncing plugins, skills, agents, MCP"
systemprompt-internal-bridge sync --allow-tofu

step "verifying"
systemprompt-internal-bridge doctor || warn "doctor reported problems — read them above before trusting the result"

# A fresh home has no ~/.claude/settings.json, and without one Claude Code
# defaults to the biggest model the gateway offers. Seed haiku as the starting
# model — only when the file is absent, so a /model choice the user saved on a
# previous run is never overridden.
if [ ! -f "$HOME/.claude/settings.json" ]; then
    step "seeding default model (haiku)"
    mkdir -p "$HOME/.claude"
    printf '{\n  "model": "haiku"\n}\n' > "$HOME/.claude/settings.json"
fi

if [ -n "${CLEAN_CLIENT_EXEC_CLAUDE:-}" ]; then
    step "starting Claude Code"
    # A login shell so the managed block in ~/.profile is sourced — that
    # sourcing is what puts ANTHROPIC_BASE_URL in front of claude.
    exec bash -lc claude
fi

cat <<'DONE'

  Ready. Type:

      claude

  This shell is a login shell, so ANTHROPIC_BASE_URL and ANTHROPIC_AUTH_TOKEN
  are already set from ~/.config/systemprompt-internal/env.sh. Proxy log: ~/.cache/bridge-proxy.log

DONE

# A login shell, so the managed block in ~/.profile that install just wrote is
# actually sourced — that sourcing is the thing being tested.
exec bash -l
