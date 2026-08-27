# Demo 1 — Lead Inbox Triage

The opening move of the enterprise demo: the "SDR agent" use case, done in one governed tool call.
An Agentforce SDR topic takes weeks of Flow configuration; here a skill plus one typed MCP tool does the
triage, and the platform prices the task to the cent.

This is the simplest of the five demo skills. It proves the core loop:
**skill → MCP tool → typed structured result → dashboard artifact → audited cost.**

## When to Use

- Running the enterprise demo (this is step 1 — the others are `demo_account_360`,
  `demo_followup_orchestrator`, `demo_governed_operations`, `demo_command_center`).
- Any time someone asks "show me the inbound leads" and you want the full demo framing.

## Script

1. **Fetch** — call `crm_lead_search` (no filters beyond a sensible `limit`, e.g. 20). The result is a
   typed table (`structuredContent.columns` + `items` keyed on Odoo field names) rendered inline — no
   parsing, no scraping. It runs in Odoo **as the signed-in user**: their record rules decide what comes
   back.
2. **Rank** — order the leads by expected revenue and recency; flag the single hottest lead (name it with
   its Odoo id — later demo steps drill into it). At most three sentences of narrative.
3. **Dashboard** — open the **Leads — Inbound Prospects** artifact from the Artifacts library (installed
   by `systemprompt-setup-cowork`). Point out it fetches over the same wire: same tool, same identity,
   same audit row.
4. **Cost readback** — close the loop on the platform's differentiator: every call just made is already
   in the audit spine. Show it:
   ```bash
   systemprompt infra logs trace list --limit 5        # the tool call(s), with trace ids
   systemprompt infra logs request list --limit 5      # AI requests: model, tokens, cost, latency
   ```
   State the cost of this task in dollars, from the data — never an estimate.

## Talking Points

- One skill + one typed tool replaced an SDR agent build-out; time-to-demo was minutes, not weeks.
- Per-user identity: there is no service account. A salesperson sees the salesperson's pipeline.
- The cost line is real, per-task, and queryable — not a monthly credit statement.

## Rules

- Everything presented must come from this run's tool output; ids from Odoo, costs from the logs.
- If the user is not linked to Odoo, report that and point to Profile → Link Odoo account.
- Hand off: end by offering step 2, `demo_account_360`, against the hottest lead you flagged.
