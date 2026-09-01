# Business Overview

Answer "what's going on?" with live data from Odoo. This skill produces a single, scannable brief covering the
pipeline, what's new, what's been said (notes), and what's slipping (activities). It reads as the acting user, so it
shows exactly what that user is permitted to see in Odoo — nothing more.

## When to Use

- A daily or on-demand status brief ("what's happening", "morning update", "where are we").
- Before a planning conversation, to ground decisions in the current numbers.
- When the user asks about any slice of it (just overdue items, just this week's leads) — pull the full data once and
  present the slice.

## Tools

| Tool | Provides |
|------|----------|
| `business_overview_data` | The aggregate: leads by stage (count + expected revenue), new leads last 7 days, 20 most recent notes/messages, overdue and due-today activities |
| `crm_lead_report` | Deeper pipeline cuts (by owner, custom date ranges) when the brief raises questions |
| `crm_lead_get` / `crm_lead_search` | Drill into any lead the brief surfaces |
| `activity_list` | Full activity list when the overdue section needs expansion |
| `note_search` | Chase a topic the brief surfaced across every record's chatter |

## Brief Structure

Always this order, always short:

1. **Pipeline** — one line per stage: count and expected revenue. Call out deltas the data supports (e.g. new leads
   this week) — never invent trends you can't see.
2. **New** — leads created in the last 7 days: name, source if present, owner.
3. **Recent notes** — the conversation of the business: who logged what, on which record, newest first. Summarize;
   don't paste full note bodies unless asked.
4. **Needs attention** — overdue activities first (owner, record, how overdue), then due today.
5. **Suggested actions** — at most 3, each tied to a specific record surfaced above (e.g. "Lead 42 has had no activity
   in 14 days — schedule a follow-up").

## Rules

- Everything in the brief must come from the tool output of this run. No memory of previous briefs as fact.
- Name records with their Odoo ids so follow-up actions (update, note) are unambiguous.
- If a section is empty, say so in one line — an empty "needs attention" section is good news worth stating.
- If the user is not linked to Odoo, report that and point to Profile → Link Odoo account; do not fabricate a brief.
