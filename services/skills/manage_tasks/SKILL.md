# Manage Tasks

Project tasks are Odoo's shared to-do list: named work items in a project, with assignees, deadlines, and
stages — each carrying its own chatter and attachments like every other record. Use them for delivery work
and internal projects; use activities (`schedule_activity`) for record-bound follow-ups like "call the
client".

## When to Use

- "Add a task to the onboarding project: draft the rollout plan, due Friday."
- "What's open in the Acme delivery project?"
- "Move that task to Done / reassign it to Ben / push the deadline."

## Tools

| Tool | Use for |
|------|---------|
| `task_list` | Open items by project, free-text search, deadlines |
| `task_create` | New task: name, project, assignee, deadline, description |
| `task_update` | Stage moves, reassignment, deadline changes — field passthrough like `crm_lead_update` |
| `note_add` | Log context on a task (`model: "project.task"`) — decisions, blockers |
| `attachment_add` | Attach the deliverable to the task itself |

## How to Work

1. **Project first.** `task_create` needs a real project; if the named project doesn't resolve, the tool
   lists what exists — pick with the user, don't create typo-twins.
2. **Tasks are shared state.** Write names that stand alone ("Draft Acme rollout plan", not "do the thing").
   Put the why in the description or a chatter note.
3. **Listing**: default to open tasks; group by project; flag overdue deadlines first.
4. **Activity vs task**: a promise to a customer on a lead = activity; a piece of work someone must produce
   = task. When in doubt, ask what the user wants to see it as.

## Rules

- Reassignment and deadline changes affect someone's workload — confirm before writing, name them after.
- Never mark a task done on inference; completion is the user's call.
