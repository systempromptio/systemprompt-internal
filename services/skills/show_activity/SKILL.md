# Show Activity

"What has been happening?" — answered from Odoo, live, as one scannable brief. This skill runs a
series of read-only queries as the acting Odoo user, so the brief is exactly what that person is
permitted to see: no service account, no cache, no memory of last time.

## Ask me things like

- "What's been going on this week?" / "Morning update."
- "What did the team write about Acme?" / "What's overdue?"
- "Who's carrying the pipeline?" / "What's in the calendar?"

## Prerequisite

Your platform account must be linked to your Odoo account (Profile → Link Odoo account: Odoo login +
personal API key). If a tool says you are not linked, stop and say so — never fabricate a brief.

## The queries, in order

Pull everything once, then present; slice only if the user asked for a slice.

| Step | Tool | What it gives |
|------|------|---------------|
| 1 | `business_overview_data` | Pipeline by stage (count + expected revenue), leads created in the window, the 20 most recent notes, overdue and due-today activities — one call |
| 2 | `crm_lead_search` with `limit` | The newest leads in full: name, company, stage, owner, revenue |
| 3 | `crm_lead_report` grouped by salesperson | Who is carrying what |
| 4 | `note_search` with `date_from`/`date_to` | The conversation of the business in the window; `query` a topic when the user names one |
| 5 | `calendar_event_list` with `date_from`/`date_to` | Meetings held and booked, with the records they concern |
| 6 | `task_list` | Open work, overdue deadlines first |
| 7 | `activity_list` with `overdue_only: true`, then without | Promises slipping, then promises due |
| 8 | `channel_list` | Which team channels are active — only if the user asks about Discuss |

**The window.** Default to the last 7 days. Always state the window you used in the first line —
"12 new leads this week" and "12 new leads this quarter" are different findings. If the user names a
window, use it everywhere: `date_from`/`date_to` on notes and calendar, and the same span when you
read the overview's "new leads".

## The brief

Always this order, always short:

1. **Pipeline** — one line per stage: count and expected revenue. Then the owner split from step 3.
   Call out only deltas the data supports.
2. **New leads** — name, company, source if present, owner, Odoo id.
3. **What people wrote** — the notes, newest first: who, on which record, the gist. Summarise; paste a
   body only if asked.
4. **Meetings** — held and upcoming, with the lead or partner each was about.
5. **Work in flight** — open tasks by project, overdue first.
6. **Needs attention** — overdue activities (owner, record, how overdue), then due today.
7. **Suggested actions** — at most three, each tied to a record surfaced above by name and id
   ("Lead 42 has had no note in 14 days — schedule a follow-up"). Offer to act on them with
   `update_leads`; never act unasked.

An empty section gets one line saying so — an empty "needs attention" is good news worth stating.

## Dashboards

The same data has standing views: **Business Overview**, **Leads — Inbound Prospects**, **Pipeline —
Open Deals**, and **Recent Activity**, installed into the Artifacts library by `systemprompt_setup_admin`.
They refetch over the same wire, as the same identity, landing the same audit rows. After the brief,
offer to open the one that matches what the user asked about.

## Rules

- Everything in the brief comes from this run's tool output. No numbers from memory, no trends you
  cannot see in the rows.
- Name every record with its Odoo id so the next action is unambiguous.
- A permission error from Odoo is a correct outcome: Odoo's record rules decide what this user sees.
  Report it and move on; never route around it.
- To change anything the brief surfaced, hand to `update_leads`. To say what this brief cost and how
  each call was governed, hand to `governance_readback`.
