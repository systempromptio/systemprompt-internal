# My Workspace

"What's my day?" — answered from Odoo, live, as one short brief, with the four workspace dashboards
standing behind it. Every query in this skill is read-only and runs as the acting Odoo user, so the
brief is exactly what that person is permitted to see: their own activities, the deals visible to
them, the notes they can read. No service account, no cache, no memory of last time.

## Ask me things like

- "What's on my plate?" / "Morning."
- "What am I late on?" / "What's closing this month?"
- "What did the team write this week?"
- "Open my dashboards."

## Prerequisite

Your platform account must be linked to your Odoo account (Profile → Link Odoo account: Odoo login +
personal API key). Call `mcp__odoo__activity_list` with `{ "limit": 1 }` first. A result means
linked. An authentication or missing-identity error means it is not — say so, point at
`/admin/profile`, and stop. Never fabricate a brief.

## The queries, in order

Pull everything once, then present; slice only if the user asked for a slice.

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
   the week. Open tasks after activities, overdue first. Name every record with its Odoo id.
2. **Closing soon** — open deals in close-date order: name, contact, tag(s), expected close, expected
   revenue. Group by tag when there are more than a handful. Call out deals already past their close
   date and deals with no close date at all — both are worth a decision.
3. **Said this week** — the notes, newest first: who, on which record, the gist. Summarise; paste a
   body only if asked.
4. **Suggested next step** — at most three, each tied to a record surfaced above by name and id.
   Never act unasked; this skill reads.

An empty section gets one line saying so — "nothing overdue" is good news worth stating.

## Dashboards

The same four queries have standing views, installed into the Artifacts library by
`systemprompt_setup_admin` from the `systemprompt-workspace` bundle:

| Dashboard | Backed by | Use it for |
|-----------|-----------|------------|
| **To-Do Bulletin** | `activity_list` + `task_list` | Ticking an activity done — the tick calls `activity_complete` with your feedback |
| **Upcoming Deals** | `crm_lead_search` open_only, sort deadline | Deals grouped by tag, due this week / this month / no date |
| **Pipeline — Open Deals** | `crm_lead_search` open_only | The open pipeline as a sortable table |
| **Recent Activity — Team Notes** | `note_search` | The chatter feed, searchable |

They refetch over the same wire, as the same identity, landing the same audit rows. After the brief,
offer to open the one that matches what the user asked about. If a dashboard is not installed, say
so and point at an admin: `systemprompt_setup_admin` is the only thing that installs dashboards, and
it is admin-only. Never recreate a dashboard by hand.

## Rules

- Everything in the brief comes from this run's tool output. No numbers from memory.
- Read-only. Completing an activity happens in the **To-Do Bulletin** (its tick is the only write);
  changing a lead is `update_leads`, which only admins hold — if the user is not an admin, say the
  change is theirs to make in Odoo.
- A permission error from Odoo is a correct outcome: Odoo's record rules decide what this user sees.
  Report it and move on; never route around it.
