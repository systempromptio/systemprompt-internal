---
title: "Gateway API (/v1/messages)"
description: "The governed inference gateway: the /v1/messages contract, the required x-session-id header, model allow-listing, and the three profile API URLs."
author: "systemprompt.io"
slug: "gateway-api"
keywords: "gateway, /v1/messages, x-session-id, inference, model allow-list, routes, quota, api url, governance"
kind: "guide"
public: true
tags: ["gateway", "api", "governance"]
published_at: "2026-05-19"
updated_at: "2026-09-28"
after_reading_this:
  - "Call the governed inference gateway at POST /v1/messages"
  - "Supply the required x-session-id header so a request is not rejected with HTTP 400"
  - "Understand how the gateway model allow-list rejects un-listed models with HTTP 403"
  - "Edit model routes, quota windows and safety settings from the console"
  - "Pick the right profile api_*_url for in-container vs host callers"
related_docs:
  - title: "Authentication"
    url: "/documentation/authentication"
---

# Gateway API

**TL;DR:** the gateway exposes `POST /v1/messages` — an Anthropic-Messages-compatible
endpoint that authenticates, authorises, governs, and audits every inference call before
proxying it upstream. Every request **must** carry an `x-session-id` header and an
`Authorization` (or `x-api-key`) credential. The model named in the body must be
permitted by the gateway policy allow-list, or the gateway returns `403` before any
upstream call is made.

## Request contract

`POST /v1/messages`

Required headers:

| Header | Purpose | Missing → |
|--------|---------|-----------|
| `Authorization: Bearer <jwt>` *or* `x-api-key: <key>` | Caller identity | `401 Unauthorized` |
| `x-session-id: <session id>` | Binds the call to a session for audit and conversation continuity | `400 Bad Request` ("missing required x-session-id header") |
| `x-gateway-conversation-id: <id>` *(optional)* | Pins the conversation; otherwise it is derived from the message body | — |

Body: the Anthropic Messages shape — `model`, `max_tokens`, `messages[]`.

The `x-session-id` header is **mandatory**. A request without it is rejected with
`400`, not a clearer error — supply a session id minted by the session-login flow.

## Model allow-listing

The profile's provider registry (`profile.providers`, e.g. in
`.systemprompt/profiles/local/profile.yaml`) declares every provider and the
models it serves; those model ids are the deployment's allow-list. A request
whose `model` is neither declared in the registry nor matched by a gateway route
(or absorbed by `gateway.default_provider`) is denied with `403` before any
upstream call — this is the egress control an air-gapped deployment relies on.

Both the dispatch gate (`GatewayConfig::is_model_exposed`) and the `/profile`
model list derive from the registry, so adding a model means editing the
`providers` block.

Gateway-route RBAC additionally keys on the route `id`. If the caller's role or
department is not assigned to the route (and the route is not `default_included`), the
gateway returns `403` with a message that names the route id, the model, and the
remedy.

## Routes, policies and quotas in the console

**Routes.** `/admin/gateway` shows which requested model goes to which provider, in the
order the dispatcher tries them (first match wins). The Routes tab edits that order and
each route's pattern, provider, upstream model, `name` and `description`. A save writes
the `gateway_routes` table and regenerates the `routes:` sequence of
`services/ai/gateway.yaml` from it; core dispatches from the file, so a saved route is
live at the next restart. `/admin/gateway/routes/{route_id}` shows one route and who may
use it.

**Policies.** `/admin/gateway/policies` edits `ai_gateway_policies`, the database
projection of `services/gateway/policies.yaml`: quota windows, quota and safety modes,
scanners and the categories that block. Core re-reads the table per request, so a saved
policy is live within a minute. The file stays the declaration; `/admin/sync` reports
when the two differ and offers either direction.

**Quotas.** A quota window names a subject — `user`, `group`, `project`, `role`,
`connector`, or `organization` (the whole installation, subject id `default`) — a period
and ceilings on requests, tokens and cost. `/admin/governance/quotas` shows each
subject's usage in the current period against those ceilings. Under `quota_mode: warn`
(this instance's setting) a subject past a ceiling is recorded as a warning and the
request still runs; under `enforce` it is answered `429` until the window turns over.

## Profile API URLs

A profile declares three server URLs; pick the right one for the caller:

| Field | Use it for |
|-------|------------|
| `api_internal_url` | Service-to-service calls inside the deployment network |
| `api_server_url` | The address the server binds / in-container CLI calls |
| `api_external_url` | The public, host-facing address; used for OAuth/WebAuthn URL generation and consumed by `session login` |

When the host-published port differs from the in-container port (as in the air-gap
stack), an in-container caller must use the in-container address and host callers must
use the published one — a single value cannot satisfy both. The air-gap profile pins
all three to the in-container `:8080` and automated host callers pass the published
port explicitly.
