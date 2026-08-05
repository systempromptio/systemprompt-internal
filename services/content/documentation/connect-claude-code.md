---
title: "Connect Claude Code"
description: "From an empty machine to a working, governed Claude Code session. Clone, build, start the server, and connect with a one-shot code — plus how to verify it from a clean state and how to sign out."
author: "Astound Digital"
slug: "connect-claude-code"
keywords: "claude code, connect, bridge, one-shot code, exchange code, just claude, setup local, developer setup, clean state"
kind: "guide"
public: true
tags: ["documentation", "getting-started", "claude-code"]
published_at: "2026-08-06"
updated_at: "2026-08-06"
after_reading_this:
  - "Stand up the gateway on your own machine from an empty clone"
  - "Connect Claude Code with a single command and a one-shot code"
  - "Verify the whole flow from a genuinely clean state"
  - "Know what the connect step writes, and how to undo it"
---

# Connect Claude Code

This is the developer path: an empty machine to a Claude Code session whose
every request goes through your gateway and lands in the audit trail.

The whole connect step is one command. Everything before it is standing up the
server that command talks to.

## Before you start

You need Docker, [`just`](https://github.com/casey/just), a Rust toolchain, and
an API key for one AI provider. You do **not** need a checkout of
`systemprompt-core` — this repository builds against the published release.

## 1. Download

```bash
git clone https://github.com/systempromptio/systemprompt-astound.git && cd systemprompt-astound
```

## 2. Build

`setup-local` writes a local profile, starts a Docker Postgres scoped to this
clone, runs the migrations, and publishes the site. Called with no key it asks
which provider you want; pass one to keep it non-interactive.

```bash
just setup-local          # interactive provider pick
just build
```

The first build compiles the full dependency graph and takes a while. Later
builds are incremental.

## 3. Start the server

```bash
just start
```

The dashboard is now on <http://localhost:8080>.

## 4. Connect Claude Code

Sign in to the dashboard and open **Profile**. The connect card mints a
**one-shot code** and prints the command with the code already filled in:

```bash
just claude <code>
```

That runs Claude Code in a throwaway container, connected to your gateway. Your
own machine is not reconfigured — nothing to source, nothing to undo. The first
run also builds the client, so it takes a few minutes; later runs start
immediately.

To sign that session out and start from nothing:

```bash
just claude-reset
```

### What the code actually is

The dashboard never shows you a long-lived credential. It issues a one-shot
exchange code: 32 random bytes, stored only as a hash, valid for ten minutes,
usable once. The client redeems it for a durable token that stays on the machine
it was issued to. A code caught in a screenshot or a shell history is worthless
within minutes; a token never travels through the browser at all.

An administrator can issue the same code from the CLI, which is how unattended
and headless setups work:

```bash
systemprompt admin bridge issue-code --user-id <email-or-uuid>
```

## Configuring your own machine instead

If you want `claude` on your host rather than in a container — the right choice
for daily work on a machine you own:

```bash
just connect <code>
```

Be aware of what that writes, because it is not confined to a container:

| Path | What it is |
|------|-----------|
| `~/.config/astound/` | Client config, the token (mode 0600), the loopback key |
| `~/.profile` | A managed block that sets `ANTHROPIC_BASE_URL` and the auth token |
| `~/.claude/managed-settings.json` | Base URL, `apiKeyHelper`, model discovery |
| `~/.local/share/Claude/org-plugins/` | Your organization's plugins, skills, and MCP servers |
| systemd user units | A 30-minute sync timer and the loopback inference proxy |

Open a new login shell (or `. ~/.profile`) afterwards, then run `claude`.

On a machine with no checkout of this repository, the installer does the same
thing directly:

```bash
curl -fsSL https://your-gateway/files/downloads/install.sh | sh -s -- \
  --download-base https://your-gateway/files/downloads --code <code>
```

## Verifying the flow from a clean state

Worth doing after any change to the connect path, because the failure mode is
silent: a machine that already holds a valid token sails through the sign-in
step and proves nothing.

Clone into a new directory, with no profile and no sibling checkout, and give it
ports of its own so it cannot collide with a gateway you already run:

```bash
git clone https://github.com/systempromptio/systemprompt-astound.git fresh && cd fresh
just setup-local <provider-key> "" "" 8081 5436
just build
just start
```

The fresh database has no users, so make one and issue it a code:

```bash
systemprompt admin users create --email you@example.com --if-not-exists
systemprompt admin users role promote you@example.com
systemprompt admin bridge issue-code --user-id you@example.com
```

Then clear any stored session before connecting — **this is the step that makes
the test meaningful** — and connect against the test port:

```bash
just claude-reset
just claude <code> http://localhost:8081
```

Read the output rather than trusting the exit code. It must say **"signing in
with the supplied code"**. If it says *"already signed in — reusing the stored
PAT"*, a previous session survived, the sign-in never ran, and the test result
is meaningless.

`astound-bridge doctor` runs at the end and prints a pass/fail line per check.
One warning is expected and benign: the hook-token check reports no OAuth client
yet, because provisioning is lazy — it happens on the first plugin hook request,
not during sync.

## Troubleshooting

**"Client not built yet"** — expected on a first run; `just claude` builds it.
The client is a separate workspace, so a plain `just build` does not produce it.

**The code is rejected** — codes last ten minutes and are single-use. Reload the
profile page for a fresh one.

**Claude Code answers but nothing appears in the audit trail** — the session is
not going through the gateway. Check `ANTHROPIC_BASE_URL` points at the loopback
proxy, and that `astound-bridge doctor` reports the inference proxy running.

**A container cannot reach the gateway** — inside a container `localhost` is the
container. `just claude` rewrites it to the host alias for you; if you are
running the installer by hand, use `http://host.docker.internal:8080`.
