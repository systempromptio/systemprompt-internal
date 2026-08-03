---
title: "Step 1 — Salesforce App Setup"
description: "Create the Salesforce External Client App, configure the callback URL and OAuth scopes, and point the platform at your org's My Domain so users can sign in with Salesforce."
author: "Astound Digital"
slug: "salesforce-app-setup"
keywords: "salesforce, connected app, external client app, consumer key, oauth scopes, my domain, callback url, sso, pkce"
kind: "guide"
public: true
tags: ["salesforce", "sso", "oauth", "setup"]
published_at: "2026-07-29"
updated_at: "2026-07-31"
after_reading_this:
  - "Create a Salesforce app with the correct OAuth scopes and callback URL"
  - "Configure services/web/config/salesforce.yaml against your own org"
  - "Provision SALESFORCE_CLIENT_SECRET without committing it"
  - "Diagnose every ?sso= failure reason on the login page"
related_playbooks:
  - title: "Start Here — Standing Up the Gateway"
    url: "/documentation/use-case-admin"
  - title: "Overview"
    url: "/documentation/salesforce"
  - title: "Step 2 — JWT-Bearer and the Signing Certificate"
    url: "/documentation/salesforce-jwt-bearer"
  - title: "Step 4 — Users, Seats and Roles"
    url: "/documentation/salesforce-provisioning"
---

# Step 1 — Salesforce App Setup

**TL;DR:** Create one Salesforce app, copy its consumer key into
`services/web/config/salesforce.yaml`, put the consumer secret in
`SALESFORCE_CLIENT_SECRET`, and set the callback URL to exactly
`https://<your-host>/admin/auth/salesforce/callback`. This step delivers
"Sign in with Salesforce" on the login page.

## Before You Start

You need Salesforce Setup access sufficient to create an app and read My Domain,
and you need to know the public HTTPS hostname of your deployed platform
instance. The callback URL must match character-for-character on both sides, so
settle the hostname first.

## A Note on App Types

Salesforce is mid-migration from **Connected Apps** to **External Client Apps**.
Both work here — the OAuth flow is identical and the platform only ever sees a
consumer key and secret. New orgs should prefer External Client Apps, since
Salesforce has stated Connected Apps are the legacy path. The repository's
comments use both terms interchangeably; wherever you read "Connected App",
"External Client App" is equally valid.

## 1. Find Your My Domain

In Salesforce Setup, search for **My Domain** and copy the **Current My Domain
URL**. It looks like `https://yourcompany.my.salesforce.com`.

This one value does triple duty in the platform: it is the OAuth host, the
federated-identity **issuer** key recorded against every linked account, and the
`aud` of the JWT-bearer assertion in step 2. Changing it later re-keys every
existing federated identity, so use the production org's My Domain from the
start rather than migrating from a sandbox value.

## 2. Create the App

In Setup, create a new External Client App (or Connected App) and enable OAuth.

**Callback URL** — set it to exactly:

```
https://<your-host>/admin/auth/salesforce/callback
```

**OAuth scopes** — select the four that map to what the platform requests:

| Scope | Why it is needed |
|-------|------------------|
| `openid` | Issues the ID token that identifies the user |
| `email` | Supplies the `email` and `email_verified` claims the gate depends on |
| `profile` | Supplies `name` and, critically, `preferred_username` — the Salesforce Username step 2 signs assertions with |
| `api` | Covers direct REST calls against the org |

Do **not** add `refresh_token`. The platform deliberately does not request it:
Salesforce tokens for tool calls are minted fresh on demand via the JWT-bearer
grant rather than banked at login, so there is no long-lived token to refresh
and nothing to leak.

Save the app, then copy the **Consumer Key** and **Consumer Secret**. Salesforce
may take several minutes to propagate a new app before the first login succeeds.

## 3. Configure the Platform

Edit `services/web/config/salesforce.yaml`:

```yaml
enabled: true

# Setup → My Domain → Current My Domain URL
my_domain: "https://yourcompany.my.salesforce.com"

# The app's Consumer Key. Not a secret — safe to commit.
client_id: "3MVG9..."

# Must match the app's callback URL exactly.
redirect_uri: "https://your-host/admin/auth/salesforce/callback"

scopes: "openid email profile api"

# Only these verified email domains may sign in.
allowed_email_domains:
  - yourcompany.com

auto_provision: true
```

Every key is validated strictly — an unknown key fails the load loudly rather
than being silently ignored.

Two fields deserve thought:

**`allowed_email_domains`** is a hard gate applied *after* Salesforce
authenticates the user and *before* any account is created. A user with a
verified email outside these domains is rejected with `?sso=forbidden` and no
local account is minted. The shipped file includes `systemprompt.io` for
platform testing — remove it.

**`auto_provision`** decides your provisioning posture. Left `true`, the first
verified allow-listed login creates the account with zero admin steps — the
frictionless path. Set to `false`, SSO will only log in or link users an admin
has already created, and an unknown identity is rejected with
`?sso=not_provisioned`. Step 4 covers the trade-off.

## 4. Provision the Secret

The consumer secret is never stored in `salesforce.yaml`. The platform resolves
it from the `SALESFORCE_CLIENT_SECRET` environment variable first, then falls
back to the `salesforce_client_secret` key in the active profile's gitignored
`secrets.json`.

```bash
export SALESFORCE_CLIENT_SECRET='<your consumer secret>'
```

Use your normal secret-management path for the deployed instance. If the secret
is missing, login fails with `?sso=unavailable` and an error is logged — the
routes still mount, so the failure is clean rather than a crash.

## What the Login Flow Does

Understanding the sequence makes the failure reasons below self-explanatory.

1. The user clicks **Sign in with Salesforce**, hitting
   `/admin/auth/salesforce/start`. Any `?redirect=` target is carried through.
2. The platform generates a PKCE verifier and S256 challenge, and stores the
   state, verifier, and redirect target in a short-lived `sf_oauth_state`
   cookie — `HttpOnly`, `SameSite=Lax`, scoped to the callback path, 10-minute
   lifetime.
3. The browser is redirected to `{my_domain}/services/oauth2/authorize`.
4. Salesforce authenticates the user, applying your org's MFA and login policies.
5. Salesforce redirects to `/admin/auth/salesforce/callback`. The platform
   validates the state, exchanges the code for tokens, and fetches
   `{my_domain}/services/oauth2/userinfo`.
6. The claims are gated, the identity is resolved to a local user, and the
   Salesforce Username is recorded.
7. A platform session JWT is minted and set as an `access_token` cookie.

The Salesforce access token from this exchange is used only to fetch userinfo.
It is never retained.

## Verify

Restart the server, then:

```bash
# The login page should render the Salesforce button
curl -s https://<your-host>/admin/login | grep -o 'salesforce-login'

# Watch the flow live while you sign in from a browser
systemprompt infra logs view --follow --since 30s
```

Sign in as a test user whose email is on an allow-listed domain. Success lands
you on `/admin` with an `access_token` cookie set. A failure returns you to
`/admin/login?sso=<reason>` with a message in the error banner.

## Troubleshooting

Every failure collapses to one `?sso=` reason on the login URL:

| Reason | Meaning | Fix |
|--------|---------|-----|
| `unavailable` | `SALESFORCE_CLIENT_SECRET` is unset, or the config is incomplete or disabled | Provision the secret; confirm `enabled: true` and that `my_domain`, `client_id`, and `redirect_uri` are all non-empty |
| `denied` | The user cancelled at the Salesforce consent screen | None — user action |
| `forbidden` | The email's domain is not in `allowed_email_domains` | Add the domain, or have the user sign in with their work account |
| `unverified` | Salesforce reported `email_verified: false` | The user verifies their email in Salesforce. This gate is deliberate: linking an unverified address would let a hostile identity provider claim an existing account through the email-merge path |
| `no_email` | Salesforce returned no email claim | Confirm the `email` scope is on the app and the user record has an email |
| `not_provisioned` | `auto_provision: false` and no existing account matches | An admin creates the account first — see step 4 |
| `seat_limit` | The user's organization has used every seat | Free a seat or raise the plan limit — see step 4 |
| `error` | Token exchange or userinfo fetch failed | Check the server logs. Usually a `redirect_uri` mismatch or a stale consumer secret |

A `redirect_uri` mismatch is by far the most common first-run failure, and
Salesforce's own error text for it is unhelpfully generic. Compare the two
values byte for byte, including the scheme and any trailing slash.

Next: **[Step 2 — JWT-Bearer and the Signing Certificate](/documentation/salesforce-jwt-bearer)**.
