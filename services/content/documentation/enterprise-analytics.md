---
title: "Usage, Adoption & Productivity Analytics"
description: "Track AI usage, adoption, and productivity in the admin analytics tabs: request volume, weekly active users, and honestly labelled code metrics, split by project."
author: "systemprompt.io"
slug: "enterprise-analytics"
keywords: "analytics, usage, adoption, productivity, wau, projects, code metrics, dashboards"
kind: "guide"
public: true
tags: ["enterprise", "analytics", "admin"]
published_at: "2026-08-25"
updated_at: "2026-09-11"
after_reading_this:
  - "Navigate the Overview, Usage, and Code analytics tabs"
  - "Switch between period presets from 15 minutes to 30 days, or set a custom range"
  - "Read WAU and requests per user per day, per project"
  - "Interpret the Code tab's productivity proxies for what they are"
  - "Understand what stands between usage data and an outcome metric"
related_docs:
  - title: "Cost Management, Budgets & FinOps"
    url: "/documentation/enterprise-cost-management"
  - title: "Dashboard"
    url: "/documentation/dashboard"
---

# Usage, Adoption & Productivity Analytics

**TL;DR:** `/admin/analytics` is a server-rendered dashboard with tabs for Overview, Usage, and Code (plus Spend, covered in [Cost Management](/documentation/enterprise-cost-management)). It answers who is using the platform, how much, and what they produce — with selectable periods from 15 minutes to 30 days, and every view filterable by project or user.

## Periods and filters

Every tab shares the same controls: period presets from **15 minutes up to 30 days**, plus a **custom range** picker, and a **Project** selector. A person's project comes from their project membership — see [User & Access Management](/documentation/enterprise-user-access) — and an admin can narrow any page to one; a plain user only ever sees their own.

## Overview and Usage tabs

The Overview and Usage tabs cover platform utilization:

- **Request volume** — total requests over the period, with daily and weekly series and historical trend.
- **Error rate** — failed requests as a share of total.
- **Active users** — distinct users per bucket, charted over time.

For CLI cross-checks and scripted reporting:

```bash
systemprompt analytics overview
systemprompt analytics requests stats
systemprompt analytics costs breakdown --by user --since 7d
systemprompt analytics conversations list --since 7d --source gateway
```

## Adoption

The Usage tab also measures adoption rather than raw traffic:

- **Weekly active users (WAU)** with period-over-period deltas.
- **Requests per user per day** — the intensity metric: is usage broad and shallow, or concentrated?
- **Top users** — a leaderboard of the heaviest users in the period.

## Code tab: productivity proxies

The Code tab reports what Claude Code sessions produce. These metrics are **proxies, and are labelled as such in the UI** — they indicate direction, not ground truth:

- **AI-authored lines of code** — LOC written by the model in observed sessions.
- **Applied edits** — edits the model proposed that were applied.
- **Permission-grant rate** — how often users approve the tool actions the model requests; a rough trust signal.
- **Commit lines** — lines landing in commits observed through Claude Code sessions, deduplicated and rolled up daily beside AI usage.

Two limits are worth stating plainly:

- **Tab-acceptance rate is not measurable.** Claude Code emits no accept/reject signal for completions, and no manual-LOC baseline exists, so a true acceptance metric would require an IDE-level integration that does not exist today.
- **Commits made outside Claude Code are invisible.** Full commit analytics require nominating an authoritative SCM and an identity mapping into it.

Both limitations have their canonical home on the [Enterprise Roadmap](/documentation/enterprise-roadmap).

## Verification

Use the commands and UI checks above against the configured instance. Repository maintainers can run `just test-unit` and `just test-integration`; a passing fixture test does not establish that a deployed integration is configured.

## From usage to outcomes

Everything above measures **consumption**. Mapping consumption to **outcomes** is a separate question, and it needs a decision before it needs engineering.

### What is measured today

Every request through the gateway records input tokens, output tokens, cache-read and cache-creation tokens, cost, latency, model, provider, and the user, session and trace it belongs to. Those rows roll up per user per day, which is what the dashboards and the per-user page at `/admin/analytics/users/{user_id}` read.

Two honest caveats. **Cost is computed, not billed** — it comes from a model price catalogue applied to observed token counts, so it is an accurate estimate rather than an invoice. And the customer-facing report deliberately omits cost entirely; spend lives in the internal report, so that a report shared with a team shows usage without exposing budget.

### The limit worth stating

The only outcome-shaped data captured today is **self-reported by the model**: at the end of a Claude Code session a summariser records whether the stated goal was achieved, a quality score, and a goal-to-outcome mapping. It is a useful coaching signal and a reasonable way to spot sessions that went badly. It is not a business metric, and it should not be presented as one — it is a model's opinion of its own work, not an observed result in a system of record.

Nothing today joins AI usage to a ticket being closed, a deal advancing, an incident being resolved, or a release shipping.

### The decision required

To map usage to outcomes we first have to agree what an outcome *is*. The candidates, in rough order of how directly they are observable:

- **Source control** — pull requests merged, review turnaround, change failure rate. Closest to delivery, but only meaningful once commits made outside Claude Code are visible.
- **Odoo records** — leads advanced, deals won, quotations raised. The right measure for commercial rather than engineering usage; Odoo is already connected, so the open question is the identity mapping, not credentials.
- **Ratified self-report** — keep the session goal signal, but have a human confirm or reject it, converting an opinion into a label.

Whichever is chosen, two dependencies follow and neither is optional:

1. **Access to that system from systemprompt.** An outcome source needs credentials before its data can be correlated with anything. Without them there is no outcome side of the join.
2. **An identity mapping rule** from the external account to a platform user — an Odoo user id, a commit-author email. This is the harder half.

Until an outcome is defined and its source system connected, the platform can report what AI cost and what it produced in code, and it should not claim more than that.
