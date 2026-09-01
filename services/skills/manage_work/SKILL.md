# Manage Work

The delivery side of the business in Odoo: **project tasks** (work someone must produce) and the
**calendar** (meetings, linked to the records they concern). Both run as the acting Odoo user, and
both carry their own chatter and attachments like every other Odoo record — so work and meetings sit
in the same knowledge graph as the pipeline, not in a silo.

## Prerequisite

Your platform account must be linked to your Odoo account (Profile → Link Odoo account: Odoo login +
personal API key). Unlinked users get a clear error from every Odoo tool — link first, then retry.

## Which of the three is it?

This is the decision that goes wrong most often, so make it explicitly:

| The user means | Use | Where it lives |
|----------------|-----|----------------|
| A piece of work someone must produce | **task** (here) | a project |
| A promise made on a customer record | **activity** (`crm` skill) | the lead or partner's chatter |
| A meeting at a time | **event** (here) | the calendar, optionally linked to a lead |

When in doubt, ask what the user wants to see it as. "Draft the Acme rollout plan" is a task; "call
Acme Friday" is an activity; "demo with Acme Thursday 2pm" is an event.

## Tools

| Tool | Use for |
|------|---------|
| `task_list` | Open items by project, free-text search, deadlines |
| `task_create` | New task: name, project, assignee, deadline, description |
| `task_update` | Stage moves, reassignment, deadline changes — field passthrough like `crm_lead_update` |
| `calendar_event_list` | Agenda queries: date range, or search by name |
| `calendar_event_create` | Book: name, start (+ stop or duration), attendees, linked record |
| `partner_search` | Resolve attendees to partner ids |
| `crm_lead_search` | Resolve the lead a task or meeting is about, to link it |
| `note_add` | Log context on a task (`model: "project.task"`) — decisions, blockers |
| `attachment_add` | Attach the deliverable to the task itself |

## Tasks

1. **Project first.** `task_create` needs a real project; if the named project does not resolve, the
   tool lists what exists — pick with the user, never create typo-twins.
2. **Tasks are shared state.** Write names that stand alone ("Draft Acme rollout plan", not "do the
   thing"). Put the why in the description or a chatter note.
3. **Listing**: default to open tasks; group by project; flag overdue deadlines first.
4. **Reassignment and deadline changes affect someone's workload** — confirm before writing, name
   them after.
5. **Never mark a task done on inference.** Completion is the user's call.

## Calendar

1. **Reading**: default to today or this week; present time, title, location, attendees, and the
   linked record.
2. **Booking**: confirm date, time, and duration explicitly — never book from a vague time. Resolve
   attendees via `partner_search`; link the lead when the meeting is about a deal, which puts the
   meeting on that lead's timeline.
3. **Time zones**: state times back in plain terms ("Thursday 14:00"); if the user's timezone is
   unclear and it matters, ask.
4. **Never move or double-book over an existing event without saying so.** Meetings you create are
   visible to attendees immediately — book only what was actually agreed.
5. **After the meeting**: log the outcome as a note and raise an activity on the lead for whatever
   was promised — both live in the `crm` skill. A meeting that produces no record and no follow-up is
   a smell.

## Output Style

Answer with the facts from Odoo: task ids, names, projects, assignees, deadlines; event times,
attendees, and linked records. When you changed something, state exactly what changed (field,
old → new) and on which id.
