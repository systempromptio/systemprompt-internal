# Systemprompt Setup — Codex CLI

Get OpenAI Codex CLI routing its inference through the systemprompt.io gateway, so every request is
authenticated, audited, and attributed like any other governed agent. The heavy lifting is done by
the desktop bridge — this skill walks the user through it and verifies the result. Safe to re-run:
every step checks before it changes anything.

## Ask me things like

- "Set up Codex with systemprompt."
- "Route Codex through the gateway."
- "Is my Codex CLI governed?"

## Step 1 — Install and sign in to the bridge

The bridge manages host-app profiles; Codex CLI is one of its supported hosts. If it is not
installed yet, download the installer for the user's OS. The asset names are version-less on
purpose, so these stay permanent links:

| OS | Asset |
|----|-------|
| macOS (Apple Silicon **and** Intel — one universal build) | `systemprompt-internal-bridge-macos.dmg` |
| Linux x86_64 | `systemprompt-internal-bridge-linux-x86_64.tar.gz` |
| Linux aarch64 | `systemprompt-internal-bridge-linux-aarch64.tar.gz` |
| Windows x86_64 | `systemprompt-internal-bridge-windows.exe` |

```bash
# NOTE: the GitHub release may lag the deployed gateway — prefer the assets at
# https://internal.systemprompt.io/files/downloads/ where hosted (Windows,
# Linux x86_64); macOS is GitHub-only and may need a fresh bridge-v* release.
curl -LO https://github.com/systempromptio/systemprompt-internal/releases/latest/download/systemprompt-internal-bridge-macos.dmg
```

`systemprompt-internal` is a **private repository**, so a plain `curl` only works for someone with
repo access — otherwise use `gh release download --repo systempromptio/systemprompt-internal
--pattern 'systemprompt-internal-bridge-macos.dmg'` while authenticated with `gh`, or have the
user's admin send them the installer. On macOS, open the dmg and drag **Systemprompt Internal
Bridge.app** to `/Applications`; the full walkthrough is `docs/install/bridge-macos.md`.

Then have them sign in with their systemprompt account (Odoo credentials or passkey). Signing in
links the device through `/bridge-auth/device-link` automatically.

## Step 2 — Enable the Codex CLI agent

In the bridge's agents step (or Settings → Agents), enable **Codex CLI**. The bridge writes the
managed profile itself — the model provider in `~/.codex/config.toml` pointing at the gateway and
the credential to authenticate with. Never hand-edit that file to point at the gateway; the bridge
re-syncs managed keys and will flag manual drift as stale.

## Step 3 — Verify a governed request

Have the user run any small Codex CLI prompt, then confirm the request landed in the audit spine:

```bash
systemprompt infra logs request list --limit 5
```

A row with the user's identity appearing right after the Codex run means the wiring is complete.
No row means Codex is still talking to OpenAI directly — re-check Step 2 (the bridge's Codex card
should read "Installed", not "stale" or "unmanaged").

## What this does NOT do — dashboards

Codex gets **no dashboards**, and this is not a gap to work around. Codex has no artifact library:
no `create_artifact`, no `list_artifacts`, no persistent gallery, and no CLI equivalent —
`coworkctl` does not exist. Its one HTML surface, the inline visualization, renders into a
thread-scoped scratch directory and is explicitly blocked from calling tools (`callMcp` rejects
with "Inline visualizations cannot call tools", and the page's CSP sets `connect-src 'none'`), so a
dashboard rendered there could never fetch Odoo data.

So: do not stage dashboard HTML anywhere on this host, do not go looking for an install command,
and do not write a setup receipt reporting zero installs. Say the dashboards live in Claude Cowork
and are installed there by `systemprompt_setup_cowork` (or `admin_workspace_setup_cowork` for the
admin control-plane set).

## What this does NOT do — Odoo identity

Odoo access from Codex flows through
the gateway's MCP surface and the user's linked Odoo identity; if Odoo tools fail with a
missing-identity error, the user connects Odoo on their profile page (`/admin/profile`).
