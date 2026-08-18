---
title: "Connect Odoo"
description: "Stand up the Odoo backend and link users to it: instance requirements, server configuration, per-user API keys, and how permissions and audit flow through to Odoo."
author: "systemprompt.io"
slug: "odoo"
keywords: "odoo, setup, api key, json-rpc, crm, leads, notes, activities, permissions, oidc"
kind: "guide"
public: true
tags: ["documentation", "odoo", "setup"]
published_at: "2026-08-06"
updated_at: "2026-08-06"
after_reading_this:
  - "Know which Odoo edition and apps the platform needs"
  - "Configure the server's ODOO_URL and ODOO_DB"
  - "Link a user's Odoo account with a personal API key"
  - "Understand why every AI action carries the real user's identity in Odoo"
---

# Connect Odoo

Odoo is the system of record. The platform never keeps its own copy of leads,
notes, or activities — every read and write goes to Odoo over JSON-RPC, and
every call is executed **as the acting user**, so Odoo's own access rules and
audit trail apply to everything the AI does.

## Instance requirements

- **Odoo 16 or later, self-hosted Community or Enterprise.** All APIs used here
  (JSON-RPC, per-user API keys, `crm.lead`, `mail.message`, `mail.activity`)
  ship in Community. Avoid Odoo Online (SaaS): it blocks custom/OCA modules,
  which rules out the OIDC single sign-on upgrade path.
- **Apps installed:** CRM (`crm`). Discuss and Activities come with base.
- Network reachability from this platform to the Odoo host over HTTPS.

## Server configuration

Two values, set as environment variables (or in the profile's secrets):

```bash
ODOO_URL=https://odoo.example.com   # base URL, no trailing /jsonrpc
ODOO_DB=production                  # database name
```

There is no server-wide Odoo credential. The server holds only the address;
identity comes from each user's link.

### Local development

`just setup-local` writes an Odoo CE sidecar (odoo:18) into the same Docker
compose stack as the local Postgres, so `just db-up` starts both. One-time
init after that:

```bash
just db-up
just odoo-local-init   # creates the odoo role + odoo_local DB (base module, no demo data)
```

Odoo then answers on http://localhost:8070 (override with the sixth
`setup-local` argument), login `admin` / `admin`. setup-local seeds
`odoo_url` / `odoo_db` into the profile's `secrets.json`; the server reads
them via the secrets bootstrap and the MCP spawner injects them into the
odoo MCP server's environment, so no `.env` file is involved. Logs:
`just odoo-local-logs`; restart: `just odoo-local-restart`.

## Link a user

Each user connects their own Odoo account once:

1. In Odoo: avatar → My Profile → Account Security → **New API Key**. Copy the
   key — Odoo shows it once.
2. In this platform: **Profile → Link Odoo account** — enter your Odoo login
   and paste the API key.
3. The platform validates the pair against Odoo's `authenticate` endpoint,
   stores the key encrypted, and records your Odoo user id.

Unlink at any time from the same page; unlinking revokes nothing in Odoo, so
also delete the API key there if the machine is being retired.

## What per-user execution buys you

- **Permissions are Odoo's.** A user who cannot see a record in Odoo cannot
  reach it through the AI either — the refusal comes from Odoo, not from a
  parallel rule set that could drift.
- **Audit is native.** Notes are authored by the real user in the record's
  chatter; `create_uid`/`write_uid` on every touched record are genuine.
- **No shared bot account** accumulating god-mode access.

## Single sign-on (optional, later)

With self-hosted Odoo you can install the OCA `auth_oidc` module and point
both Odoo and this platform at one identity provider. That replaces the manual
API-key link with automatic identity mapping — no change to skills or tools.

## Troubleshooting

| Symptom | Cause |
|---|---|
| "Odoo account not linked" from any tool | The user skipped the link step — Profile → Link Odoo account |
| Link fails immediately | Wrong login/key, or the key was created for a different database than `ODOO_DB` |
| Permission error on a specific record | Working as designed — the user lacks access in Odoo |
| Every call times out | `ODOO_URL` unreachable from the server; check network and TLS |
