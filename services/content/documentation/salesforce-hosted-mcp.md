---
title: "Step 3 — Hosted MCP Access"
description: "Activate Salesforce's Hosted MCP server, register its endpoint, and understand how the platform injects a per-user Salesforce bearer into every tool call."
author: "Astound Digital"
slug: "salesforce-hosted-mcp"
keywords: "salesforce hosted mcp, mcp_api, soqlQuery, external mcp server, external_auth, sobject-all, token accessor, gateway proxy"
kind: "guide"
public: true
tags: ["salesforce", "mcp", "tools", "setup"]
published_at: "2026-07-29"
updated_at: "2026-07-29"
after_reading_this:
  - "Activate a Salesforce Hosted MCP server and copy its exact Server URL"
  - "Register the endpoint in services/mcp/salesforce.yaml"
  - "Understand the external_auth path that injects a per-user Salesforce bearer"
  - "Confirm a live tool call runs as the signed-in Salesforce user"
related_playbooks:
  - title: "Overview"
    url: "/documentation/salesforce"
  - title: "Step 2 — JWT-Bearer and the Signing Certificate"
    url: "/documentation/salesforce-jwt-bearer"
  - title: "Step 4 — Users, Seats and Roles"
    url: "/documentation/salesforce-provisioning"
---

# Step 3 — Hosted MCP Access

**TL;DR:** Turn on a Salesforce Hosted MCP server in Setup, copy its exact
Server URL into `services/mcp/salesforce.yaml`, and the platform will call it
with a freshly minted per-user bearer on every tool call.

## What Salesforce Hosted MCP Is

Salesforce runs the MCP server itself, inside Salesforce, as a first-party
OAuth resource server. Nothing is deployed on your side and no data is
replicated. A tool call arrives at Salesforce carrying a user's bearer token,
and Salesforce executes it as that user — full object and field permissions,
sharing rules, and record access applied by the same engine that governs the UI.

That is why this design is worth the setup cost. Permissions are not
reimplemented, approximated, or configured twice. They are simply Salesforce's.

Hosted MCP is not available on every Salesforce edition. Confirm availability
with your Salesforce account team before planning a rollout.

## 1. Activate the Server in Salesforce

In Salesforce Setup, find the MCP server configuration, activate the server you
want, and **copy its exact Server URL**. Do not construct the URL by hand.

Server URLs follow this pattern:

```
production:  https://api.salesforce.com/platform/mcp/v1/platform/<server>
sandbox:     https://api.salesforce.com/platform/mcp/v1/sandbox/<server>
```

Note the `platform` versus `sandbox` segment — it is the single most common
copy-paste error, and a sandbox URL in a production config fails in a way whose
error text does not mention the environment at all.

Salesforce offers several toolsets as separate servers. This repository is set
up for `sobject-all`, the general-purpose SObject toolset.

## 2. Confirm the Scope

The Hosted MCP endpoint requires the bearer to carry the `mcp_api` scope.

You do not add this to the login scopes in `salesforce.yaml`. The bearer used
for tool calls is not the login token — it is the JWT-bearer token minted in
step 2, and its scopes come from the app's own configuration in Salesforce.
Ensure `mcp_api` is among the OAuth scopes on the app you created in step 1.

If tool calls return `403` while login works fine, this scope is the first thing
to check.

## 3. Register the Endpoint

Edit `services/mcp/salesforce.yaml` and set the endpoint you copied:

```yaml
mcp_servers:
  salesforce:
    type: external
    binary: ""
    package: null
    port: 5020
    endpoint: "https://api.salesforce.com/platform/mcp/v1/platform/sobject-all"
    enabled: true
    display_in_web: true
    description: "Salesforce Hosted MCP — SObject CRUD/SOQL (runs as the signed-in user)"
    external_auth:
      token_endpoint: "/api/public/salesforce/token"
      header: "Authorization"
      scheme: "Bearer"
    oauth:
      required: false
      scopes:
      - user
      audience: mcp
      client_id: null
```

Three fields are easy to misread:

**`binary: ""` and `port: 5020`** are unused. An external server is never
spawned locally — it is reached at `endpoint`. The port is nominal and the empty
binary path is never executed.

**`external_auth`** is the field that makes the whole integration work. Its
presence switches the platform onto the per-user credential path. Without it,
the platform would forward its *own* JWT to Salesforce, which Salesforce would
reject. With it, the platform calls `token_endpoint` with the user's JWT, gets
back that user's freshly minted Salesforce bearer, and injects it as
`Authorization: Bearer <token>`.

**`oauth.required: false`** is deliberate, not an oversight. This block is the
*platform's* gate — who on this platform may reach this server — and it is left
non-gating because `external_auth` already governs authentication. Setting it
would double-gate the same call. Access control for who may use Salesforce tools
belongs in step 4's role rules, not here.

## How a Tool Call Flows

```
Bridge → gateway: call tool, with the user's platform JWT
gateway → /api/public/salesforce/token, with that JWT
         ↳ looks up the user's Salesforce Username
         ↳ signs an RFC 7523 assertion, exchanges it
         ↳ returns { access_token, instance_url }
gateway → api.salesforce.com/…/sobject-all, Authorization: Bearer <that token>
Salesforce → executes as that user, returns the result
gateway → audits the call and returns it to the bridge
```

The token accessor authenticates its caller by accepting a platform JWT bearing
either the API or MCP audience. That is deliberately broader than the
cookie-based admin paths, because the caller here is the gateway acting on
behalf of an MCP session rather than a browser.

If a mint fails, the accessor returns `502` — an upstream fault, not a platform
one. That distinction matters when triaging: a `502` on this endpoint always
means Salesforce refused, so investigate step 2 rather than the platform.

## Available Tools

The `sobject-all` toolset exposes an 11-tool set covering SOQL and SObject CRUD.
We have verified one tool name against a live org:

| Tool | Argument | Purpose |
|------|----------|---------|
| `soqlQuery` | `q` | Run a SOQL query |

The remaining tools cover record retrieval, creation, update, deletion, object
schema description, and search. **We have not verified their exact names against
a live org**, so treat any list of them as provisional and read the tool list
your own server advertises rather than trusting documentation — including this
page. The `soqlQuery` entry above is the one to rely on; note in particular that
its argument key is `q`, not `query`.

To discover the real list from your own org, connect the bridge and inspect the
tools it advertises for the `salesforce` server.

## Adding More Toolsets

Each Salesforce Hosted toolset is a separate server, so each gets its own entry:
same `external_auth` block, different name and `endpoint`. Add the new name to
the `mcp_servers.include` list of whichever plugin should expose it.

Only `sobject-all` is registered today. The metadata, Agentforce, and models
toolsets each need their exact Setup Server URL before they can be added.

## Verify

```bash
# Is the server registered and reachable?
systemprompt plugins mcp logs salesforce

# After running a tool from the bridge, confirm the call was audited
systemprompt infra logs trace list --limit 10
systemprompt infra logs trace show <trace-id>
```

The definitive test is a live query. From a bridge session signed in as a real
Salesforce user, run a `soqlQuery` such as
`SELECT Id, Name FROM Account LIMIT 5`, then confirm two things: the rows come
back, and they are **that user's** accounts. If two users with different
Salesforce visibility get identical results, per-user identity is not working —
suspect a fallback to a shared credential and revisit step 2.

## Troubleshooting

| Symptom | Likely cause |
|---------|--------------|
| `401` from Salesforce | The bearer was rejected. Usually the platform forwarded its own JWT because `external_auth` is missing or malformed |
| `403` from Salesforce | The bearer lacks the `mcp_api` scope, or the user lacks permission on the object |
| `502` from the token accessor | The mint failed — see step 2 |
| Server not listed in the bridge | `enabled: false`, or the user's role has no grant — see step 4 |
| Empty results for everything | The query is fine but the user has no record access. Confirm in the Salesforce UI as that user |
| Endpoint errors mentioning an unknown server | The Server URL was hand-constructed or uses the wrong `platform`/`sandbox` segment |

Next: **[Step 4 — Users, Seats and Roles](/documentation/salesforce-provisioning)**.
