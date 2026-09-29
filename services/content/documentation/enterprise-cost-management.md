---
title: "Cost Management, Budgets & FinOps"
description: "Attribute gateway usage to users, projects, and models; inspect spend warnings and export cost reports."
author: "systemprompt.io"
slug: "enterprise-cost-management"
keywords: "cost, budget, finops, spend, caps, alerts, forecasting, csv, digests, chargeback"
kind: "guide"
public: true
tags: ["enterprise", "finops", "admin"]
published_at: "2026-08-25"
updated_at: "2026-09-11"
after_reading_this:
  - "Read the Spend tab: totals, trends, and model distribution"
  - "Read the spend warning thresholds and why nothing is refused on cost"
  - "Attribute any request's cost to its user, project, model, and provider"
  - "Export cost reports as CSV from the web UI or CLI"
related_docs:
  - title: "Usage, Adoption & Productivity Analytics"
    url: "/documentation/enterprise-analytics"
  - title: "Model Gateway, Routing & Data Residency"
    url: "/documentation/enterprise-model-routing"
  - title: "Enterprise Roadmap & Known Limitations"
    url: "/documentation/enterprise-roadmap"
---

# Cost Management, Budgets & FinOps

**TL;DR:** Every AI request lands with its cost attributed to a user, project, model, and provider. The gateway's quota windows watch spend per user and per day for the whole instance and, since 2026-09-04, only warn: a window past its threshold lands in the governance warnings report and the request still runs. The Spend tab shows totals, trends and the cost-by-model split, filterable by project; reports export as CSV from the web and CLI.

## Per-request attribution

Cost tracking is not a rollup bolted on afterwards: **each request row carries user, model, provider, tokens, and cost** (stored as microdollar integers end to end), and users can belong to multiple projects through dashboard membership. Dashboards and reports slice on all of these dimensions in near real time, so chargeback to a project is a filter, not a reconciliation project.

```bash
systemprompt analytics costs summary
systemprompt analytics costs breakdown --by user --since 30d      # spend, requests, tokens, conversations per user
systemprompt infra logs request list --limit 20 --user <user-id>
```

## The Spend tab

`/admin/analytics` includes a Spend tab showing:

- **Total spend and daily cost trend** for the selected period, with cost per request and token counters.
- **Model distribution and cost-by-model series** — where the money actually goes.
- **Project selector** — every figure on the tab narrows to one project.

## Spend warning thresholds

Spend thresholds are the gateway's **quota windows** (`services/gateway/policies.yaml`): a per-user hourly window sized to flag a runaway agent, and an instance-wide daily window sized to flag an unusually expensive day. The quota plane runs in **warn mode** (`quota_mode: warn`, set on 2026-09-04): a window past its threshold is recorded as a `warn` decision under policy `quota`, shows up on the governance warnings report at `/admin/governance/warnings` and in `systemprompt infra logs governance report`, and the request is never refused. There is no spend clamp on this instance. Costs are attributed one request late by design (a request's cost is known after its response), so the warning lands on the request after the crossing.

Switching a window back to enforcement (`quota_mode: enforce`, HTTP 429 with `retry-after` once a window is spent) is a review-visible change: a unit test pins the mode and the thresholds.

### Subjects, periods and the two pages

A window names a **subject** — whose usage it counts — and a **period**. The subjects this instance resolves are `user` (core's own), `group`, `project` and `organization` (the whole installation, subject id `default`); each is a `SubjectAttributeProvider` in `extensions/web/admin/src/authz/`, and a person on several groups or projects is counted against the primary one from their scope defaults, the same one their cost is attributed to. `quota_fault_mode: closed` means a window whose subject cannot be resolved refuses the request rather than passing it uncounted.

Periods are an hour, a day, a week, any number of seconds, or a **calendar month**. Core's windows are fixed-length and aligned to the Unix epoch, so a calendar month is an interim the admin extension provides: the window is declared as `window_seconds: 2678400` (31 days), and the daily `quota_month_window` job rewrites the live row to run out at the month's end and carries yesterday's bucket into today's, so the bucket the gateway reserves against holds month-to-date usage and the first of the month starts from zero. The one gap is the first minute after UTC midnight, while core's policy cache is stale.

Two console pages read and write the same table, `ai_gateway_policies`:

- **`/admin/gateway/policies`** edits the windows, the safety scanners and block lists, and the warn/enforce switch of each plane. Core re-reads the table on every request (a sixty-second cache), so a save is live without a restart or a file edit. `/admin/sync` then shows the edit as drift on the `gateway_policies` plane until it is exported to `services/gateway/policies.yaml` and committed, or overwritten from code.
- **`/admin/governance/quotas`** shows usage against ceiling per subject for every window in force — the one bucket per subject the gateway reserves against in the current period, with a meter for the ceiling closest to being spent. Under warn mode a subject past 100% was warned about, never refused.

## Self-service reporting

- **CLI** — cost reports accept `--since` / `--until` and export CSV. Start from `systemprompt analytics costs summary`; `analytics costs breakdown --by user|model|provider|agent` slices the same window, and `systemprompt analytics --help` lists the rest.
- **Web** — the admin UI exposes CSV export endpoints for usage and request reports, narrowed to the project you select.

## Provider and model cost comparison

An internal report (also reachable from the CLI) breaks cost and margin down **by provider and by model**, so you can see what a switch of route would save. Quality-normalized comparison ("is the cheaper model good enough?") requires a quality baseline for your workloads — see the [roadmap](/documentation/enterprise-roadmap).

## Verification

Use the commands and UI checks above against the configured instance. Repository maintainers can run `just test-unit` and `just test-integration`; a passing fixture test does not establish that a deployed integration is configured.
