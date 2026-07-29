---
title: "Salesforce Integration Overview"
description: "How an enterprise connects its own Salesforce org to the Astound bridge: the five setup steps, the trust chain from SSO login to a per-user Hosted MCP call, and what a provisioned Salesforce user experiences."
author: "Astound Digital"
slug: "salesforce"
keywords: "salesforce, sso, oauth, jwt bearer, hosted mcp, bridge, enterprise setup, connected app, provisioning"
kind: "guide"
public: true
tags: ["salesforce", "integration", "sso", "bridge"]
published_at: "2026-07-29"
updated_at: "2026-07-29"
after_reading_this:
  - "Understand the trust chain that lets a Salesforce user reach their own Salesforce data through the bridge"
  - "Know the five setup steps and which team owns each one"
  - "Know what is configured in Salesforce Setup versus what is configured in this repo"
  - "Know the current known limits before planning a production rollout"
related_playbooks:
  - title: "Step 1 — Salesforce App Setup"
    url: "/documentation/salesforce-app-setup"
  - title: "Step 2 — JWT-Bearer and the Signing Certificate"
    url: "/documentation/salesforce-jwt-bearer"
  - title: "Step 3 — Hosted MCP Access"
    url: "/documentation/salesforce-hosted-mcp"
  - title: "Step 4 — Users, Seats and Roles"
    url: "/documentation/salesforce-provisioning"
  - title: "Step 5 — Rolling Out the Bridge"
    url: "/documentation/salesforce-bridge-rollout"
---

# Salesforce Integration Overview

**TL;DR:** Point the platform at your Salesforce org once, and every provisioned
user in that org signs in with Salesforce, links the desktop bridge, and works
against your own Salesforce data. Every tool call runs as the signed-in
Salesforce user, under that user's own Salesforce permissions, and lands in the
governance audit trail.

## The Problem This Solves

Giving an AI assistant access to Salesforce usually means minting one
integration user with broad permissions and letting everyone share it. That
collapses the audit trail — every record change is attributed to the integration
user, not the person — and it hands every user the union of everyone's
permissions.

This integration does the opposite. There is no shared service account. When a
user asks the assistant to read the pipeline, the platform mints a short-lived
Salesforce token **for that specific user** and calls Salesforce's Hosted MCP
endpoint with it. Salesforce enforces that user's own object and field
permissions, sharing rules, and record access. A sales rep sees their
territory; a manager sees theirs. The assistant cannot exceed what the person
could do in the Salesforce UI.

## The Trust Chain

Five links, each verified independently:

| Link | What happens | Who enforces it |
|------|--------------|-----------------|
| 1. **Login** | The user authenticates against your Salesforce org via OAuth 2.0 + PKCE. The platform reads verified `email`, `email_verified`, and `preferred_username` claims. | Salesforce |
| 2. **Gate** | The email must be verified and its domain allow-listed. A failure never creates an account. | Platform |
| 3. **Provision** | A local account is linked or just-in-time created, consuming an organization seat. The Salesforce **Username** is recorded. | Platform |
| 4. **Mint** | On each tool call, the platform signs a short-lived RFC 7523 assertion with your app's private key, `sub` = that user's Salesforce Username, and exchanges it for a fresh Salesforce access token. Nothing is stored. | Salesforce |
| 5. **Call** | The token is injected into the request to Salesforce's Hosted MCP endpoint. Salesforce runs the tool as that user. | Salesforce |

The bridge never talks to Salesforce directly. It holds only a platform
credential; the Salesforce token is minted server-side and injected at the
gateway, so a compromised laptop never yields a Salesforce token.

## What You Configure Where

| In Salesforce Setup | In this repo |
|---------------------|--------------|
| External Client App (or Connected App) with OAuth enabled | `services/web/config/salesforce.yaml` |
| Callback URL, OAuth scopes | `services/mcp/salesforce.yaml` |
| Digital certificate for JWT signing | `services/access-control/plans.yaml` |
| Pre-authorized profiles or permission sets | `services/access-control/roles.yaml` |
| Hosted MCP server activation | `SALESFORCE_CLIENT_SECRET`, `SALESFORCE_PRIVATE_KEY` |

Neither secret is ever committed. Both are read from the environment first,
then from the active profile's gitignored `secrets.json`.

## The Five Steps

1. **[Salesforce App Setup](/documentation/salesforce-app-setup)** — create the
   app, get the consumer key, set the callback URL and scopes, and point the
   platform at your org's My Domain.
2. **[JWT-Bearer and the Signing Certificate](/documentation/salesforce-jwt-bearer)**
   — generate a keypair, upload the certificate, pre-authorize users, and
   provision the private key.
3. **[Hosted MCP Access](/documentation/salesforce-hosted-mcp)** — activate the
   Salesforce Hosted MCP server and register its endpoint.
4. **[Users, Seats and Roles](/documentation/salesforce-provisioning)** — claim
   your email domains, set a plan and seat limit, and decide what a default user
   can reach.
5. **[Rolling Out the Bridge](/documentation/salesforce-bridge-rollout)** —
   install the bridge, link a device, and verify the first live query.

Steps 1–3 are typically a Salesforce administrator working alongside a platform
engineer. Steps 4–5 are the platform team.

## What a Provisioned User Experiences

1. Installs `astound-bridge` and opens it.
2. Clicks **Sign in with Salesforce**. The browser opens the platform login,
   which redirects to your Salesforce org.
3. Authenticates with their normal Salesforce credentials — including whatever
   MFA your org enforces. No new password.
4. Approves the device link. The bridge receives its credential.
5. The bridge syncs the marketplace and the Salesforce tools appear.

First run to first query is a couple of minutes, with no admin involvement per
user, provided their email domain is allow-listed and a seat is free.

## Prerequisites

- A Salesforce org with My Domain enabled and Hosted MCP available. Hosted MCP
  is not available on every edition — confirm with your Salesforce account team
  before planning a rollout.
- Permission to create an External Client App (or Connected App) and to upload
  a certificate to it.
- A deployed platform instance reachable over HTTPS at a stable hostname.
- Users whose Salesforce email addresses are verified and on a domain you
  control.

## Known Limits

These are true of the repository as it currently ships. Address each before a
production rollout.

1. **`redirect_uri` and the bridge's default gateway URL are both
   `http://localhost:8080`.** Both must become your deployed HTTPS hostname —
   see steps 1 and 5.
2. **`my_domain` points at a Dev Edition org.** Swap it for your production My
   Domain.
3. **Only the shipped organizations' domains are claimed.** Two organizations
   are defined in `services/access-control/plans.yaml` — Astound Digital and
   systemprompt.io — both on the `enterprise` plan. Any SSO arrival on a domain
   outside those lands unattached: no organization, no plan grants, and **no
   seat enforcement**. Adding your own organization is step 4, and it is the
   single most important step to not skip.
4. **Only the `sobject-all` Hosted MCP toolset is registered.** The metadata,
   Agentforce, and models toolsets each need their own entry once you have their
   exact Server URLs.
5. **The JWT-bearer `sub` silently falls back to the login email** when no
   Salesforce Username was captured for a user. In orgs where Username differs
   from email — which is most orgs — the mint fails for that user. See step 2.
6. **`allowed_email_domains` ships with `systemprompt.io` included** for
   platform testing. Trim it to your own domains.

## Verify the Whole Chain

Once all five steps are done, a single end-to-end check proves every link:

```bash
# 1. Is Salesforce SSO configured and reachable?
systemprompt infra logs view --level error --since 1h

# 2. Sign in at /admin/login via "Sign in with Salesforce", then confirm
#    the Hosted MCP server is healthy and listed
systemprompt plugins mcp logs salesforce

# 3. From the bridge, run any Salesforce tool, then confirm it was audited
systemprompt infra logs trace list --limit 5
```

A trace row naming the signed-in user, the `salesforce` server, and a successful
result means all five links are working.
