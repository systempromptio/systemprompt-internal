# Demo 5 — Command Center

The finale, and the reason the previous four steps kept reading costs back: the "cheaper, faster" claim
is now provable from this installation's own metered data. This is the analytics + billing use case —
Agentforce answers it with a credit statement priced per conversation; here every task decomposes into
requests, tokens, tools, and dollars, queryable to the row.

Admin-only. The `analytics_dashboards`, `admin_activity_report`, and `inspect_ai_requests` skills carry
the deep command references — lean on them; this skill is the demo sequence.

## Script

1. **Business close** — `crm_lead_report` grouped by stage: the pipeline as it stands after the demo's
   writes (the lead from step 1, the activity and meeting from step 3 are all in there).
2. **Fleet view** — the platform's own operations:
   ```bash
   systemprompt analytics overview --since 24h
   systemprompt analytics costs summary
   systemprompt analytics costs breakdown --by model
   systemprompt analytics tools stats
   systemprompt analytics requests stats
   ```
   Narrate: spend, request count, cache-hit rate, tool reliability, latency — one paragraph.
3. **Dashboards** — open the three admin artifacts from the library: **Usage & Costs**,
   **Activity & Requests**, **Users Directory**. Same data plane, standing views.
4. **The bill for this demo** — total what the audience just watched:
   ```bash
   systemprompt infra logs request list --since 2h     # every AI request of the session, priced
   systemprompt analytics costs trends --since 24h
   ```
   Sum the demo session's requests and state it plainly: *"Everything you just saw — triage, a 360
   brief, four CRM writes, three governance denials, and this report — cost $X.XX."*
5. **The comparison** — set that number against Agentforce's list pricing (approximately $2 per
   conversation on Flex Credits, per Salesforce's published pricing — cite it as list price, and let
   the audience correct it if they have negotiated rates). Typically the whole demo costs less than
   one Agentforce conversation. Add the structural points: self-hosted (flat infra cost, no per-seat
   Einstein SKUs), per-user identity, and an audit row for every cent.

## Rules

- Every number spoken must come from a command run in this session — the comparison only lands if our
  side of it is verifiably real. Never estimate our costs.
- Label the Salesforce figure as list pricing, not a measurement.
- Close by pointing at DEMO.md for the runbook and at the governance revert if step 4 enabled the
  stages.
