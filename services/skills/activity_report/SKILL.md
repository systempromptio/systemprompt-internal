# Activity Report

What has been happening, read live from Odoo — either across the whole business or narrowed to your
own day. Read-only, always: this skill never creates or changes an Odoo record. Every query runs
**as you** (the linked Odoo user), so a brief shows exactly what you are permitted to see — no
service account, no cache, no memory of last time. To act on anything a brief surfaces — create a
lead, move a stage, log a note, resolve a follow-up — hand off to `manage_leads`, or to `pending_task`
for a guided sweep of everything outstanding.

## Ask me things like

- "What's been going on this week?" / "Morning update." / "Who's carrying the pipeline?"
- "What's on my plate?" / "My day." / "What am I late on?" / "What's closing for me?"
- "What did the team write about Acme?"
- "Open my dashboards."

## Prerequisite

Your platform account must be linked to your Odoo account (Profile → Link Odoo account: Odoo login +
personal API key). If a tool says you are not linked, stop and say so — never fabricate a brief.

## Which mode

| Ask | Mode |
|---|---|
| "What's happening?" / "morning update" / pipeline-wide, team, or company scope | 1 — Business brief |
| "What's on my plate?" / "my day" / "what am I late on?" / "what's closing for me?" | 2 — My day |

---

# 1. Business brief

"What has been happening?" across the whole business, answered live from Odoo. A series of
read-only queries as the acting user, so the brief is exactly what that person is permitted to see.

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
6. **Needs attention** — overdue activities (owner, record, how overdue), then due today. If there is
   more than a couple, offer `pending_task` to sweep them one by one rather than listing every row here.
7. **Suggested actions** — at most three, each tied to a record surfaced above by name and id
   ("Lead 42 has had no note in 14 days — schedule a follow-up"). Offer to hand off to `manage_leads`
   (a single named change) or `pending_task` (a guided sweep); never act unasked.

An empty section gets one line saying so — an empty "needs attention" is good news worth stating.

---

# 2. My day

"What's on my plate?" — the same live-Odoo discipline, narrowed to the acting user's own activities,
tasks, and deals, plus what the team wrote this week.

## Prerequisite check

Call `activity_list` with `{ "limit": 1 }` first. A result means linked. An authentication or
missing-identity error means it is not — say so, point at `/admin/profile`, and stop.

## The queries, in order

| Step | Tool | Arguments | What it gives |
|------|------|-----------|---------------|
| 1 | `activity_list` | `{ "overdue_only": true, "limit": 100 }`, then `{ "limit": 100 }` | Your promises — the ones already slipping, then the rest due. The tool pins the acting user, so this is never anyone else's list |
| 2 | `task_list` | `{ "open_only": true, "limit": 100 }` | Open project tasks with deadlines, overdue first |
| 3 | `crm_lead_search` | `{ "open_only": true, "sort": "deadline", "limit": 100 }` | Open opportunities, closest expected close first, each with its tags and expected revenue |
| 4 | `note_search` | `{ "query": "%", "date_from": <7 days ago>, "date_to": <today>, "limit": 50 }` | What the team wrote this week — including the chatter the brain@ pipeline posted on approval |

**The window.** Default the notes to the last 7 days and say so in the first line. If the user names a
window, use it on `note_search` and when you decide what "closing soon" means.

## The brief

Always this order, always short:

1. **Owed** — overdue activities first (record, how many days late), then due today, then the rest of
   the week. Open tasks after activities, overdue first. Name every record with its Odoo id. If there
   is more than a couple, offer `pending_task` to sweep them one by one instead of listing every row.
2. **Closing soon** — open deals in close-date order: name, contact, tag(s), expected close, expected
   revenue. Group by tag when there are more than a handful. Call out deals already past their close
   date and deals with no close date at all — both are worth a decision.
3. **Said this week** — the notes, newest first: who, on which record, the gist. Summarise; paste a
   body only if asked.
4. **Suggested next step** — at most three, each tied to a record surfaced above by name and id.
   Hand off to `manage_leads` or `pending_task` to act on one; never act unasked here.

An empty section gets one line saying so — "nothing overdue" is good news worth stating.

## Dashboards

Both briefs have standing views, installed into the Artifacts library by `systemprompt_setup_admin` —
the one installer on the instance, admin-only. **Business Overview** and **Leads — Inbound
Prospects** back the business brief (Mode 1); **Pipeline — Open Deals**, **Upcoming Deals**,
**Recent Activity — Team Notes**, and **To-Do Bulletin** back the personal brief (Mode 2). All six
ship with this skill's plugin. They refetch over the same wire, as the same identity, landing the
same audit rows. **To-Do Bulletin**'s tick is the only place completing an activity happens outside
`manage_leads`/`pending_task` — it calls `activity_complete` directly. After either brief, offer to
open the dashboard that matches what the user asked about. If a dashboard is not installed, say so and
point at an admin: `systemprompt_setup_admin` is the only thing that installs dashboards, and it is
admin-only. Never recreate a dashboard by hand.

## Rules

- Everything in a brief comes from this run's tool output. No numbers from memory, no trends you
  cannot see in the rows.
- Name every record with its Odoo id so a hand-off to `manage_leads` or `pending_task` is unambiguous.
- A permission error from Odoo is a correct outcome: Odoo's record rules decide what this user sees.
  Report it and move on; never route around it.
- For cost and audit questions about anything run in this skill, hand to `demonstrate_governance`'s
  readback.
