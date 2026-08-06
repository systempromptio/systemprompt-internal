---
title: "Authentication"
description: "How identity works: CLI-provisioned accounts with passkeys for platform sign-in, and per-user Odoo credentials for CRM access. Covers sessions, JWTs, and route protection."
author: "systemprompt.io"
slug: "authentication"
keywords: "authentication, login, passkey, webauthn, session, JWT, security, provisioning, odoo identity, api key"
kind: "guide"
public: true
tags: ["authentication", "security", "login"]
published_at: "2026-03-02"
updated_at: "2026-08-06"
after_reading_this:
  - "Provision a user from the CLI and enrol their passkey"
  - "Link an Odoo login and API key so tool calls run as that person"
  - "Understand how session cookies and JWTs govern authenticated access"
  - "Know which admin routes are public and which require a session"
related_playbooks:
  - title: "Start Here — Standing Up the Gateway"
    url: "/documentation/use-case-admin"
  - title: "Connect Claude Code"
    url: "/documentation/connect-claude-code"
  - title: "Dashboard Usage"
    url: "/documentation/dashboard"
  - title: "Gateway API"
    url: "/documentation/gateway-api"
---

# Authentication

**TL;DR:** There is one way into the platform and no self-service registration.
Accounts are created from the **CLI** and each one enrols a **passkey** through
a one-shot setup link. No passwords are created or stored anywhere. Access to
CRM data is a separate, second credential: each user links their own **Odoo
login and API key** on their profile page, and every tool call runs as that
person in Odoo.

## Why There Is No Registration Page

Self-service registration was removed. It created a way for an account to exist
with no authority behind it, and every account it made had to be reconciled
against an organization and a seat afterwards — reconciliation that had no
automatic path and no owner.

Provisioning is now an explicit act by someone who already operates the
platform. That is one door, with one authority behind it.

## Platform Sign-In: CLI + Passkey

Accounts are created out-of-band. There is no way to self-provision one.

```bash
# 1. Create the account
systemprompt admin users create --name "Jane" --email jane@systemprompt.io

# 2. Grant the admin role (omit for a plain user)
systemprompt admin users role promote jane@systemprompt.io admin

# 3. Mint a one-shot passkey setup link
systemprompt admin users webauthn generate-setup-token --email jane@systemprompt.io
```

The third command prints a copy-paste URL of the form
`{api_external_url}/auth/link-passkey?token=…`, valid for 15 minutes by default
(`--expires-minutes` to change). Send it through a channel you already trust;
the user opens it, creates a passkey, and signs in with that passkey from then
on.

Roles are **not** carried in the session token. They are read from the user
record on every request, so promoting or demoting someone takes effect on their
next request — no sign-out, no waiting for a token to refresh. That is
deliberate: revocation is only worth having if it is immediate. The OAuth
*scope* minted into the JWT is fixed at issue time, so a change that widens the
scope itself still needs a fresh sign-in.

### Passkey Sign-In

Passkey authentication uses public-key cryptography. The browser generates a key
pair bound to this domain; the private key stays on the device or in the user's
password manager. The server stores only the public key, verifies a signed
challenge, and issues an OAuth 2.0 session token via PKCE.

### Lost Passkey

There is no self-service recovery — magic links were removed, and no email
service is configured to deliver them. Someone who loses passkey access needs
another operator to mint a fresh setup link with the same
`generate-setup-token` command. Keep more than one admin account so this is
never a single point of failure.

## CRM Access: Linking an Odoo Identity

Signing in to the platform does not grant access to business data. Odoo is the
system of record, and the Odoo MCP server holds **no service account**. Every
JSON-RPC call it makes is issued with the calling user's own Odoo login and API
key.

Each user links their own credential once, from `/admin/profile`:

1. In Odoo, open **Preferences → Account Security** and generate an API key.
2. On the profile page, enter the Odoo login and that key, and submit.
3. The platform calls `common.authenticate` against the configured database
   before storing anything. A credential Odoo rejects is never persisted.
4. On success the row lands in `odoo_identity`: the login, the `odoo_uid`
   returned by Odoo, and the API key encrypted with ChaCha20-Poly1305 under the
   deployment master key.

Connection settings — `ODOO_URL` and `ODOO_DB` — come from the deployment
environment, not from a user. A database name and a host belong with the
install; the keys that authenticate against them are per person.

Three consequences worth stating plainly:

- **Odoo's record rules decide what an agent can see.** A salesperson's agent
  sees that salesperson's pipeline because Odoo says so, not because the server
  filtered anything.
- **Odoo's audit log names a real person** against every note posted and every
  lead changed. There is no shared integration user to hide behind.
- **A user who has not linked Odoo gets a clear error** naming the profile page,
  not an empty result set.

Unlinking removes the row. The user keeps their platform account and loses CRM
tool access until they link again.

| Credential | Created by | Grants |
|---|---|---|
| Passkey | An operator, via CLI setup link | Sign-in to the console and the gateway |
| Odoo login + API key | The user, in Odoo, linked on their profile | Whatever Odoo already lets that person do |

## Session Management

| Property | Value |
|----------|-------|
| **Cookie name** | `access_token` |
| **Token format** | JWT |
| **Default expiry** | 3600 seconds (1 hour) |
| **Cookie flags** | `path=/`, `HttpOnly`, `SameSite=Lax`, `Secure` on HTTPS |
| **Required scopes** | `user` or `admin` |

Every admin request passes through two middleware layers. **User context
middleware** extracts and validates the JWT, then loads the user's roles and
department into a `UserContext`. **Auth check middleware** rejects protected
routes without a valid user ID, returning HTTP 401.

`UserContext` carries `user_id`, `username`, `email`, `roles`, `department`, and
`is_admin`.

To sign out, clear the `access_token` cookie.

## Public vs. Protected Routes

| Route | Access |
|-------|--------|
| `/admin/login` | Public |
| `/admin/auth/passkey/*` | Public — passkey registration and authentication |
| `/auth/link-passkey` | Public — consumes a one-shot setup token |
| `/admin/api/profile/odoo*` | Any valid session — link, unlink, status |
| `/admin/profile`, `/admin/settings`, `/admin/setup` | Any valid session, including a plain `user` |
| `/admin/*` (everything else) | Requires a valid session **and** the `admin` role |
| `/admin/enterprises*`, `/admin/reports/internal` | Requires platform admin |
| `/bridge-auth/*` | Requires a valid session |

Anonymous requests to a protected route are redirected to
`/admin/login?redirect=…`. A signed-in user **without** the `admin` role is not
shown a 403 for console pages — they are redirected to `/admin/profile`, which
is the only part of the console addressed to them. JSON admin API routes return
HTTP 403 instead, and the platform-admin routes return an HTML 403.

## System-originated actions

Every action recorded by the platform — including scheduled jobs, hooks, and MCP-server invocations — traces to a real `users` row. There is no separate "system user" or synthesized principal. The platform refuses to attribute work to an invented identity.

### How ownership is declared

Each scheduled job in `services/scheduler/config.yaml` carries an explicit `owner:` field naming an existing admin user:

```yaml
- name: publish_pipeline
  extension: web
  owner: admin
  schedule: "0 */15 * * * *"
  enabled: true
```

At startup the scheduler resolves `owner:` to a `users.id`. If the named user does not exist or is inactive, startup fails loudly — the platform refuses to run with unowned jobs. To change ownership, edit the YAML and restart.

### How attribution flows

The resolved owner becomes `JobContext.actor` for every `execute()` call. Job implementations consume it through `ctx.actor()` and pass it to any audit-row write. Governance audit rows carry three fields that together give full forensic clarity:

| Column | Meaning |
|--------|---------|
| `user_id` | The accountable principal — a real `users.id`. |
| `actor_kind` | The surface that ran the action: `user`, `job`, `mcp`. |
| `actor_id` | A label for that surface (job name, MCP server name, etc.). |

A direct human action shows as `(user_id = alice, actor_kind = 'user', actor_id = 'alice')`. A scheduled job owned by Alice shows as `(user_id = alice, actor_kind = 'job', actor_id = 'publish_pipeline')`. Same accountability column, different surface, queryable separately:

```sql
SELECT actor_kind, user_id, COUNT(*)
FROM governance_decisions
GROUP BY actor_kind, user_id;
```

### Why no separate "system" user

A dedicated "system" identity would be either a synthesized principal (impersonation) or a backdoor account with no real human accountability. Neither passes the "every action traces to a real user" bar. The designated owner is a normal admin who legitimately authorized the platform's existence by installing it — same accountability model as a unix crontab. Compromising the designated owner is exactly as bad as compromising that admin's credentials directly; there is no additional power and no amplification path.
