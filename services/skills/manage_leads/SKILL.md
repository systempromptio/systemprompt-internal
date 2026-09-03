# Manage Leads

Act on the pipeline in Odoo: create a new lead from a conversation, or walk your open leads to a
status — a stage move, a revenue number, a note, a follow-up. Odoo is the system of record
throughout. Every write runs **as you** (the linked Odoo user), so it carries real ownership and
audit history — no service account. To read what has been happening instead, use `activity_report`;
for a guided, one-by-one sweep of everything outstanding across leads, activities, and tasks, use
`pending_task`.

## Ask me things like

- "Add a lead: Acme, met their CTO at the conference, wants a demo."
- "Go through my leads with me." / "What's the status on Acme?" / "Move Acme to Proposal and log that
  they confirmed budget."

## Prerequisite

Your platform account must be linked to your Odoo account (Profile → Link Odoo account: Odoo login +
personal API key). If a tool says you are not linked, stop and say so — never fabricate a write.

## Which mode

| Ask | Mode |
|---|---|
| "Add a lead…" | 1 — Create a lead |
| "Go through my leads" / "what's the status on…" / "move X to…" | 2 — Walk & update leads |

---

# 1. Create a lead

Put a new lead into the pipeline from a conversation.

1. **Search before creating.** `crm_lead_search` on the company and the email; `partner_search` on the
   company. If it exists, work with that lead instead — put the new context on it with `note_add`. A
   duplicate is worse than no lead.
2. **Create with what you have; never invent.** Only the fields the user actually gave:
   `crm_lead_create` with `name` (required), `partner_name`, `email_from`, `phone`, `description`,
   `expected_revenue`. Leave `expected_revenue` unset rather than guessing. Put the substance of the
   conversation — what they want, what they objected to, what happens next — in `description`.
3. **Log the context** with `note_add` (`model: crm.lead`), and if a next step was agreed, offer
   `activity_create` with a concrete date. Never schedule one silently.
4. **Confirm with specifics**: the lead id, name, stage and owner as Odoo returned them.

---

# 2. Walk & update leads

Bring your existing leads up to date, one at a time, or apply a single named change ("move Acme to
Proposal").

## Tools

| Tool | Use for |
|------|---------|
| `crm_lead_search` | Find leads by text, `stage`, or `user` (salesperson) — your own with `user` set to you |
| `crm_lead_get` | One lead in full: stage, owner, revenue, probability, description |
| `crm_lead_update` | Change fields on a lead: `stage_id` for stage moves, `expected_revenue`, `probability`, `user_id` |
| `partner_search` / `partner_get` | Resolve the company or contact — is this already a customer? |
| `note_list` / `note_add` | Read a lead's chatter before acting; write the reasoning back onto it |
| `activity_list` / `activity_create` / `activity_complete` | The follow-ups already promised; schedule one; close one with feedback |
| `crm_lead_report` | Pipeline by stage or salesperson when the walk raises a question |

## The walk

1. **Pull the set.** `crm_lead_search` with `user` set to the acting user's name, open leads only. If
   the user named a stage or a company, narrow to it. For a single named change, skip straight to
   step 3 on that one lead.
2. **For each lead, one at a time**, in order of last touch (staleest first):
   - `crm_lead_get` for the current state, `note_list` for the last few chatter entries,
     `activity_list` for what is already scheduled.
   - Present a three-line card: **where it is** (stage, revenue, probability), **last touch** (newest
     note or activity, and how long ago), **what is scheduled** (next activity, or "nothing").
   - Ask one question: *"What's the status — what changed, and what's next?"* Then wait.
3. **Apply the answer.** Map it to the smallest set of writes:
   - A stage change → resolve the current stage from `crm_lead_get`, confirm the target stage by name
     if it is at all ambiguous, then `crm_lead_update` with `stage_id`.
   - A number change → `crm_lead_update` with `expected_revenue` and/or `probability`.
   - A handover → `crm_lead_update` with `user_id`; name the new owner back.
   - Context → `note_add`, in the user's voice, short and factual: what happened, what was decided.
   - A promise ("call them Friday") → propose a concrete date and `activity_create`; a promise kept →
     `activity_complete` with feedback that tells the team something.
   - "No change" → say so and move on; do not write a note that says nothing.
4. **Close with the ledger.** A table of every lead touched: id, name, stage old → new, revenue
   old → new, the note logged, the next activity and its date. Anything you did not change is not in it.

## Rules

- **Stage moves are explicit.** Never move a lead on inference; the user said the words.
- **Never invent Odoo field values.** An absent field is honest; a guessed one corrupts the pipeline.
- **Read before writing.** `note_list` first, so a note adds rather than repeats.
- **A permission error is a correct outcome.** Odoo's record rules decide what this user may touch.
  Report it plainly and stop; never route around it.
- **`note_add` is held for a second human when a non-admin calls it** (`require_approval`). This
  skill ships to every role, so a non-admin's note waits at `/admin/governance/approvals` while an
  admin's lands directly — say that if asked why a colleague's note waited and yours did not.
- Name every record with its Odoo id so the next action is unambiguous.
- For cost and audit questions about anything this skill did, hand to `demonstrate_governance`'s
  readback.
