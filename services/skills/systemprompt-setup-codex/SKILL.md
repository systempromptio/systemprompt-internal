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
installed yet, download the installer for the user's OS from the latest `bridge-v*` GitHub release
of this repository and have them sign in with their systemprompt account (Odoo credentials or
passkey). Signing in links the device through `/bridge-auth/device-link` automatically.

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

## What this does NOT do

Workspace dashboards (business overview, pipeline, leads) are a Claude Cowork feature installed by
`systemprompt-setup-cowork` — they have no Codex equivalent. Odoo access from Codex flows through
the gateway's MCP surface and the user's linked Odoo identity; if Odoo tools fail with a
missing-identity error, the user connects Odoo on their profile page (`/admin/profile`).
