---
title: "Model Gateway, Routing & Data Residency"
description: "Route all AI traffic through one provider-neutral gateway with ACL-enforced model access, quotas, latency SLOs, private endpoints, and declared residency requirements."
author: "systemprompt.io"
slug: "enterprise-model-routing"
keywords: "gateway, routing, models, providers, acl, quotas, residency, no-retain, shadow ai, slo"
kind: "guide"
public: true
tags: ["enterprise", "gateway", "admin"]
published_at: "2026-08-25"
updated_at: "2026-09-17"
after_reading_this:
  - "Route any Anthropic- or OpenAI-compatible client through the provider-neutral gateway"
  - "Grant and revoke model access per role or group via gateway_route ACLs"
  - "Enforce data-residency and no-retain requirements declaratively on routes"
  - "Set quota windows and latency SLO thresholds and read the breach reporting"
  - "Add a private or self-hosted endpoint as a configuration entry"
  - "Name a fallback provider on a route and read a failover back off the audit row"
related_docs:
  - title: "Cost Management, Budgets & FinOps"
    url: "/documentation/enterprise-cost-management"
  - title: "Audit Trail, Traceability & Observability"
    url: "/documentation/enterprise-audit-observability"
  - title: "Gateway API"
    url: "/documentation/gateway-api"
---

# Model Gateway, Routing & Data Residency

**TL;DR:** Configured clients send AI traffic through the provider-neutral gateway (`/v1/messages`, `/v1/responses`, four wire protocols) that rewrites models, enforces per-role access with 403s, refuses unlisted models, applies quota windows, tracks latency against an SLO, and — via governance metadata on providers and routes — refuses to send classified traffic to a provider that retains data.

## One gateway, any provider

The gateway speaks **four wire protocols** behind `/v1/messages` and `/v1/responses`, so any Anthropic-SDK or OpenAI-compatible client points at it unchanged. Routes match on model pattern and can rewrite via `upstream_model` — a client asking for `claude-*` can be transparently served by a different backend without any application change. On this instance the `gpt-*` and `gemini-*` routes are served by `gpt-oss-120b` on Cerebras this way.

## The shipped catalog

The catalog (`services/ai/providers.yaml`) and the routes (`services/ai/gateway.yaml`)
are implementation configuration shipped in the image, so every environment boots the
same model set. Four routes are declared:

| Route | Pattern | Served by |
|---|---|---|
| Claude (Anthropic API) | `claude-*` | Anthropic's own API |
| GPT-OSS 120B (Cerebras) | `gpt-oss-120b` | Cerebras |
| GPT (served by GPT-OSS on Cerebras) | `gpt-*` | `gpt-oss-120b` on Cerebras via `upstream_model` |
| Gemini (served by GPT-OSS on Cerebras) | `gemini-*` | `gpt-oss-120b` on Cerebras via `upstream_model` |

The current Claude entries are:

| Model | Input / output per million | Context | Max output |
|---|---|---|---|
| `claude-opus-5-5` | $4.00 / $20.00 | 1M | 128k |
| `claude-opus-5` | $5.00 / $25.00 | 1M | 128k |
| `claude-sonnet-5` | $3.00 / $15.00 | 1M | 128k |
| `claude-haiku-4-5` | $1.00 / $5.00 | 200k | 64k |
| `claude-fable-5-1` | $10.00 / $50.00 | 1M | 128k |
| `claude-fable-5` | $10.00 / $50.00 | 1M | 128k |

The Claude 4.x models stay in the catalog for clients pinned to them, and dated ids
such as `claude-haiku-4-5-20251001` are carried as aliases. `gateway.default_model`
is `claude-sonnet-5[1m]`: the `[1m]` suffix is Claude Code's 1M-context marker, which
the bridge seeds into Claude Code and the gateway strips before routing, so the
request reaches Anthropic as `claude-sonnet-5`. Without it Claude Code budgets 200k
for every gateway model. Because `allow_unlisted_models` is `false`, a request naming
a model the catalog does not list is refused rather than silently forwarded.

## Model access control

Access to models is a **`gateway_route` ACL matrix**, administered in the admin UI and granted per role, group or project. A request for a route the caller is not entitled to is refused with **HTTP 403**, the denial is audited, and grants are revocable with immediate effect.

Route ids are generated (`slug + fnv1a6`), so every console surface names a route by the optional `name:` its declaration carries — `name: Claude (Anthropic API)` beside `model_pattern` in `services/ai/gateway.yaml`, with a `description:` for the longer story — and derives "Claude models · Anthropic" from the pattern and provider when none is declared. A provider carries `display_name:` and `description:` in `services/ai/providers.yaml` the same way (`name` stays the id). The `/admin/gateway` **Models** tab is the picture by provider: endpoint, surface, wire, the models it serves and the routes that reach it; the route editor there writes `name` and `description` back to the file.

## Credentials

Personal access tokens carry an optional **expiry**, a **prefix** (identify a leaked token without exposing it), and immediate **revocation** from `/admin/devices/pats`. Note that a key is not its own governance subject — budgets and quotas bind to the owning user and the instance, not to the individual key (see the [roadmap](/documentation/enterprise-roadmap)).

## Shadow-AI posture

The gateway runs with **`allow_unlisted_models: false`**: a model that is not explicitly configured cannot be called, whoever asks. Combined with route entitlements and the full audit trail, everything that touches the gateway is governed. Traffic that never touches the gateway — someone calling a provider directly from their laptop — is a network-control matter for corporate IT, outside any application platform's reach.

## Latency SLOs

A **configurable latency SLO threshold** is tracked per the reporting views: breach percentage over the period, alongside **p50 and p95** latency and per-model latency data. Per-use-case SLO taxonomies and error-budget alerting are planned — see the [roadmap](/documentation/enterprise-roadmap).

## Quota windows

Quota windows are configured per user and instance-wide in `services/gateway/policies.yaml`. The shipped `quota_mode: warn` records breaches without refusing requests; it does not impose a hard spend cap. See [Cost Management](/documentation/enterprise-cost-management) for the active thresholds and enforcement option.

## Provider failover

A route may name a **`fallback_provider`**, optionally with a
**`fallback_upstream_model`** when the fallback's model id differs from the
requested one. No route on this instance declares one today: every model is served
by exactly one provider. A route that did would read:

```yaml
gateway:
  routes:
  - model_pattern: claude-*
    provider: anthropic
    fallback_provider: <second provider serving the same models>
```

**When it triggers.** The primary keeps its bounded retry budget for
transient answers (429, 503). Once that budget is spent, or the primary
answers any other **5xx**, or the **connection itself fails** (DNS, TLS,
timeout), the same governed request is re-bound to the fallback provider and
sent once more under the same retry policy. A 4xx other than 429 — a
malformed request, a refused key — never fails over: it is the request that
is wrong, not the provider. A per-provider **circuit breaker** records the
outcomes, so a primary that keeps failing is skipped outright and the
fallback tried first, without spending the retry budget on a dead upstream.

**What is re-sent.** The canonical request that governance and the safety
scanners already judged is rebuilt for the fallback's wire protocol; neither
runs again, because the content they judged is unchanged. The raw passthrough
lane is never re-sent — it is bound to the wire the client spoke.

**What the audit shows.** The `ai_requests` row keeps `provider` as the
route's primary and records the provider that actually answered in
**`served_provider`**, with the request repriced at the serving provider's
catalog rate. The route-match descriptor gains a `failover:<primary>-><fallback>`
segment. `systemprompt infra logs audit <request-id>` prints both; the
Prometheus counter **`gateway_upstream_failovers_total{from,to,reason}`**
(`reason` is `status_<code>`, `transport` or `circuit_open`) is on
`/metrics` for alerting.

**What is validated at boot.** The fallback is validated as the route the
fallback provider will actually serve: it must exist in the registry, differ
from the primary, reach a priced model (with a declared cache rate), and
satisfy the route's `requires:` block. A fallback that cannot be bound at
request time — credential, adapter, governance, pricing — never makes a
request worse off: the client sees the primary's own error, exactly as it
would without a fallback.

## Private and self-hosted endpoints

Any OpenAI- or Anthropic-compatible private endpoint is **a configuration entry, not a code change**. Declaring a provider with the `Backend` surface keeps it un-advertised — it serves routes without appearing in any public model listing — and every route is validated at boot.

## Data classification & residency

Provider and model YAML carries a **`governance:` block** with two booleans: `european` (data stays in the EU) and `no_retain` (the provider contractually does not retain prompts or completions). Routes declare what they demand via a **`requires:` block**:

```yaml
providers:
- name: anthropic
  governance:
    european: false
    no_retain: true
gateway:
  routes:
  - model_pattern: claude-*
    provider: anthropic
    requires:
      no_retain: true
```

Enforcement happens twice:

- **At boot** — the server refuses to start a route whose provider or model does not satisfy the route's `requires:` block. A misconfiguration is caught before any traffic flows.
- **At dispatch** — a request that would land on a non-satisfying provider is denied, with a policy audit row whose descriptor records the failed `requires:` condition, so the denial is explainable after the fact.

The gateway checks the declared metadata; operators must verify that endpoint configuration and provider terms justify those declarations. These booleans do not independently prove physical residency or contractual retention behavior, and no provider on this instance declares a `governance:` block yet. Classifying *data* (rather than routes) is planned — see the [roadmap](/documentation/enterprise-roadmap).

## Verification

Use the commands and UI checks above against the configured instance. Repository maintainers can run `just test-unit` and `just test-integration`; a passing fixture test does not establish that a deployed integration is configured.
