---
title: "Use Case — Standing Up the Gateway for Your Organization"
description: "The admin journey end to end: point the deployment at your Odoo database, provision users, have them link their own Odoo credentials, and hand the bridge to your team. What you own, in what order, and where each hazard lies."
author: "systemprompt.io"
slug: "use-case-admin"
keywords: "admin, operator, setup, odoo, json-rpc, api key, seats, roles, admin console, provisioning"
kind: "guide"
public: true
tags: ["admin", "setup", "odoo", "getting-started"]
published_at: "2026-07-31"
updated_at: "2026-08-06"
after_reading_this:
  - "Know the five things you own as the operator, and the order they happen in"
  - "Know which single step your users must do themselves, and why"
  - "Point the deployment at your Odoo database with the hazards understood"
  - "Read the admin console and know which roles see which parts of it"
related_playbooks:
  - title: "Authentication"
    url: "/documentation/authentication"
  - title: "Connect Claude Code"
    url: "/documentation/connect-claude-code"
  - title: "Dashboard Usage"
    url: "/documentation/dashboard"
  - title: "Setup & Authentication Walkthrough"
    url: "/documentation/demo-terminal-setup"
---

# Use Case — Standing Up the Gateway for Your Organization

**Who this is for:** the person who owns the Odoo instance and the platform
deployment. Usually one or two people, once per organization.

**What you need before you start:** admin access to a self-hosted Odoo
Community instance, shell access to the deployment, and an hour. Your team needs
none of this — they need an Odoo login and two minutes.

This page is the journey. Each step links to the reference page that covers it
in full; nothing here is restated there in more detail.

## What You Are Actually Building

One sentence: you are giving every person in your organization the ability to
ask their own CRM questions in plain English, from their own laptop, under
their own Odoo permissions, with every call recorded.

The chain that makes that true has five links, and you build them in order:

| # | Link | Where it lives |
|---|---|---|
| 1 | An Odoo instance the platform can reach | `ODOO_URL`, `ODOO_DB` in the deployment environment |
| 2 | A deployment master key that encrypts stored API keys | The profile's `secrets.json` |
| 3 | The Odoo MCP server enabled | `services/mcp/odoo.yaml` |
| 4 | Users, organizations and seats | `services/access-control/plans.yaml` |
| 5 | The bridge on your team's machines | `systemprompt-internal-bridge install` |

Nothing sensitive is stored on the user's side. The bridge holds a platform
credential and never an Odoo one, so a lost laptop costs you a revocation, not a
breach.

## Step 1 — The One Step You Cannot Do For Them

Each user generates their **own Odoo API key**, in Odoo, under
**Preferences → Account Security**, and links it on `/admin/profile`.

This is the only step that cannot be automated, and it will stay that way: an
Odoo API key is shown once, to the person who created it, and there is no API
that mints one on someone else's behalf. That is the point. The Odoo MCP server
holds no service account, so every JSON-RPC call it makes carries the calling
user's credential.

Two consequences decide whether the rest of the setup behaves as people expect:

- **Odoo's record rules are the authorization model.** A salesperson's agent
  sees that salesperson's pipeline because Odoo says so, not because the
  platform filtered anything. If someone sees too much, fix it in Odoo.
- **Odoo's audit log names the real person** on every note posted and every
  lead changed. There is no shared integration user in the trail.

A user who has not linked Odoo gets an explicit error naming the profile page —
not an empty result set that reads as "we have no leads".

Full detail: **[Authentication](/documentation/authentication)**.

## Step 2 — Point the Deployment at Your Odoo

Two settings and one secret.

`ODOO_URL` is the base URL of your Odoo instance. `ODOO_DB` is the database
name. Both live in the deployment environment, not in `services/` — a hostname
and a database name belong with the install, and the credentials that
authenticate against them are per user anyway.

The **deployment master key** encrypts every stored API key
(ChaCha20-Poly1305, nonce-prefixed) in the `odoo_identity` table. It goes in the
profile's `secrets.json` or the matching environment variable. Never in
`services/`. Rotating it invalidates every stored key and every user has to
re-link, so treat it as a real secret with a real backup.

`services/access-control/plans.yaml` claims your email domain for an
organization and sets the seat limit.

**The hazard:** the email domain list in `plans.yaml` decides which organization
a user joins. A user provisioned on a domain no plan claims lands unattached,
gets no plan grants and no seat check, and sees nothing at all. If someone
reports "I logged in and it's empty", check this before anything else.

## Step 3 — Check the Connection

```bash
systemprompt plugins mcp list                  # is the odoo server enabled?
systemprompt plugins mcp logs odoo             # what did it say on startup?
```

The Odoo server is defined in `services/mcp/odoo.yaml`, runs on port 5040, and
is granted to the `user` role. There is no admin-only tool on it; the
authorization that matters is Odoo's, per credential.

The connection is proved per user, not globally, because there is no global
credential to prove it with. The real test is the first linked user: the link
form calls `common.authenticate` against `ODOO_DB` before storing anything, so a
successful link *is* a successful connection test. A link that fails with "Odoo
rejected that login and API key" is a credential problem; one that fails with a
configuration error is `ODOO_URL` or `ODOO_DB`.

## Step 4 — Restart and Sign In

```bash
just build && just start
```

A restart is required, not just `just publish`: the Odoo connection settings are
read at startup, and `plans.yaml` is projected into access-control rules at
startup.

There is no self-service registration. Create your own account and enrol a
passkey from the CLI:

```bash
systemprompt admin users create --name "You" --email you@yourcompany.com
systemprompt admin users role promote you@yourcompany.com admin
systemprompt admin users webauthn generate-setup-token --email you@yourcompany.com
```

Open the printed link within 15 minutes, create a passkey, then sign in at
`/admin/login`. Go to `/admin/profile` and link your Odoo credential.

## Step 5 — Roles and the Console

Roles are stored on the user record and re-read from the database on **every
request**, so a change takes effect immediately — no sign-out, no waiting for a
token to expire. That is deliberate: revocation has to be instant to be worth
anything.

Promote someone from the CLI:

```bash
systemprompt admin users role promote sam@yourcompany.com admin
```

What each role sees:

| Role | Sees |
|---|---|
| `user` | Profile, settings, and device setup only. Everything else redirects to their profile |
| `admin` | The full console: access, catalog, entities, and the customer report |
| Platform admin | The above plus enterprise administration and internal reports |

The console divides into four areas. **Access** is people — users, departments,
personal access tokens, device certificates. **Catalog** is what they can use —
plugins, skills, MCP servers. **Entities** is what happened — AI requests,
sessions, tool traces, contexts. **Reports** is the rollup.

When you need to answer "what did this cost, who ran it, and was it allowed",
start at **Entities → Traces** and open the trace.

## Step 6 — Hand It to the Team

Per-person onboarding is two minutes, provided steps 1–5 are done: install the
bridge, enrol a passkey from the setup link you send them, link their Odoo
credential, approve the device link.

Bake the gateway URL in at install time so nobody has to configure anything:

```bash
systemprompt-internal-bridge install --gateway https://your-host
```

Before you distribute, check three things. Every user's email domain is claimed
by an organization in `plans.yaml`. You have seat headroom for the rollout size.
And everyone in scope actually has an Odoo user account — that last one is worth
verifying *before* rollout, because a person with a platform account and no Odoo
account gets stuck at the profile page with nothing to link.

Pilot with two or three people on deliberately different Odoo access rights and
have them run the same query. **Their results must differ.** Identical results
mean per-user identity is not working, and you should revisit step 1.

Full detail: **[Connect Claude Code](/documentation/connect-claude-code)**.

## A Second Operator, Before You Finish

There is no self-service recovery for a lost passkey and no email service
configured to deliver one, so **create a second admin account now**, while you
still have one that works.

```bash
systemprompt admin users create --name "Sam" --email sam@yourcompany.com
systemprompt admin users role promote sam@yourcompany.com admin
systemprompt admin users webauthn generate-setup-token --email sam@yourcompany.com
```

The third command prints a one-shot setup link, valid for 15 minutes. Send it
through a channel you already trust.

Full detail: **[Authentication](/documentation/authentication)**.

## Verify the Whole Chain

```bash
# The Odoo server is up and reachable
systemprompt plugins mcp logs odoo

# A real user's tool call, with identity, decision, cost and result
systemprompt infra logs trace list --limit 5
systemprompt infra logs trace show <trace-id>

# Anything that went wrong during the rollout window
systemprompt infra logs view --level error --since 1h
```

A trace naming a real signed-in user, the `odoo` server, and a successful result
is the end-to-end proof. Confirm the same write shows up in Odoo's own chatter
under that person's name — that is the half of the audit trail the platform does
not own.

## When Something Breaks

| Symptom | Cause |
|---|---|
| Signed in, user sees nothing | Their domain is not claimed by any organization in `plans.yaml` |
| Every CRM tool call errors with "link your Odoo account" | They have no row in `odoo_identity`. Send them to `/admin/profile` |
| "Odoo rejected that login and API key" | Wrong login, or a key revoked in Odoo. Regenerate under Preferences → Account Security |
| Linking fails with a configuration error | `ODOO_URL` or `ODOO_DB` is wrong, or Odoo is unreachable from the deployment |
| Two users get identical results | Per-user identity is not in play — check that both have their own linked credential, not a shared one |
| Every stored key stopped working at once | The deployment master key changed. Every user must re-link |
