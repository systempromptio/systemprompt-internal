# Pending Task

Everything you owe, in one guided sweep: overdue and due activities, open tasks, and leads that have
gone quiet — surfaced one at a time from live Odoo, as you, and written back the moment you answer.
Nothing is closed, moved, or noted without you saying so for that specific item. Run this when you
want to actually clear the list, not just see it — `activity_report` shows the same items as a
summary; this skill walks them.

## Ask me things like

- "Go through my pending items." / "Let's clear my list." / "What do I need to update?"
- "Sweep my overdue activities." / "Which leads have gone stale?"

## Prerequisite

Your platform account must be linked to your Odoo account (Profile → Link Odoo account: Odoo login +
personal API key). Confirm with `activity_list` `{ "limit": 1 }` — a result means linked; an
authentication or missing-identity error means it is not. Say so, point at `/admin/profile`, and stop.

## The sweep

Build one queue, item by item, oldest/most-overdue first across all three sources — do not do a
whole source before moving to the next; interleave by how overdue each item is so the sweep reflects
actual priority, not tool order.

| Source | Tool | Selects |
|--------|------|---------|
| Overdue activities | `activity_list` `{ "overdue_only": true, "limit": 100 }` | Your own promises already slipping |
| Due-today / upcoming activities | `activity_list` `{ "limit": 100 }` (minus the overdue set) | What's due this week |
| Open tasks | `task_list` `{ "open_only": true, "limit": 100 }` | Project work with a deadline, overdue first |
| Stale leads | `crm_lead_search` `{ "open_only": true, "user": <you>, "limit": 100 }`, then `note_list`/`activity_list` per lead to find last-touch date | Open leads with **no note and no activity in 14+ days** — a lead that has simply not moved is not stale on its own; only silence counts |

State the total queue size up front ("9 items: 3 overdue activities, 2 open tasks, 4 stale leads") and
the order you'll go in.

## Per item

1. **Show it.** One line of context: what it is, the record it's on (name + Odoo id), how overdue or
   how stale.
2. **Ask one question**, matched to the kind:
   - Activity/task → *"Is this done, or does it need to move?"*
   - Stale lead → *"What's the status — what changed, and what's next?"*
   Then wait. Never guess an answer to move the sweep along faster.
3. **Write back immediately**, before moving to the next item — do not batch writes to the end:
   - "Done" / "handled" → `activity_complete` with the feedback given for an activity; for a task,
     `note_add` on the linked record making the completion explicit (no dedicated task-complete tool).
   - "Push it" / a new date → `activity_create` with the new date, and mark the old one done or leave it, per what the user actually said.
   - A stage/number/owner change on a lead → `crm_lead_update`, same field mapping as `manage_leads`
     mode 2.
   - Context with no state change → `note_add`, in the user's voice, short and factual.
   - "Skip" / "later" → move on and record it as skipped in the closing ledger; do not write anything.
4. **Confirm the write** in one line before the next item: what changed, on which id.

## Closing ledger

End with one table: every item swept, the answer given, the write applied (or "skipped"), and the
record id. Anything skipped is still listed — a sweep that hides what wasn't answered is not honest
about what's still outstanding.

## Rules

- **One item, one question, one write — in that order.** Never present the whole queue and ask for
  bulk answers; staleness and overdue-ness change the right question per item.
- **Never invent a status.** If the user's answer is ambiguous, ask which of the mapped writes above
  they mean rather than picking one.
- **`note_add` is held for a second human when a non-admin calls it** (`require_approval`) — same as
  in `manage_leads`. Say so if a write parks instead of landing immediately.
- **A permission error is a correct outcome.** Odoo's record rules decide what this user may touch;
  report it and move to the next item.
- Stop cleanly if the user wants to break off mid-sweep — close the ledger with whatever was covered,
  not the full planned queue.
- For cost and audit questions about anything this skill did, hand to `demonstrate_governance`'s
  readback.
