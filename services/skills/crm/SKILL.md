# CRM — Odoo Leads

Manage the sales pipeline in Odoo without leaving the conversation. Odoo is the system of record: every lead you
create, update, or report on here is the same record the rest of the business sees, created and modified **as you**
(the linked Odoo user), so ownership, activity history, and audit fields are all real.

## Prerequisite

Your platform account must be linked to your Odoo account (Profile → Link Odoo account: Odoo login + personal API
key). Unlinked users get a clear error from every Odoo tool — link first, then retry.

## When to Use

- A prospect appears (email, call, referral, event) and needs to enter the pipeline.
- A lead's situation changed: new contact details, revised expected revenue, a stage move, qualification notes.
- You need pipeline answers: what's in each stage, who owns what, expected revenue, what arrived this week.

## Tools

| Tool | Use for |
|------|---------|
| `crm_lead_create` | New lead: name (required), partner_name, email_from, phone, description, expected_revenue |
| `crm_lead_search` | Find leads by text, stage, or owner |
| `crm_lead_get` | Full detail on one lead by id |
| `crm_lead_update` | Change any field, including `stage_id` for stage moves |
| `crm_lead_report` | Pipeline aggregates: count + expected revenue grouped by stage or owner, optional date range |
| `partner_search` / `partner_get` | Resolve companies/contacts (`res.partner`) to link or dedupe against |
| `note_list` | Read a lead's full chatter history before acting on it |
| `note_add` | Log a note onto the record: `model` (e.g. `crm.lead`, `res.partner`), `res_id`, `body` |
| `activity_list` | See what follow-ups are already scheduled on the record |
| `activity_create` | Schedule one: record (`model` + `res_id`), summary, deadline, assignee |
| `activity_complete` | Mark a follow-up done, with feedback that lands in the chatter |
| `attachment_list` | See what documents are already on the record |
| `attachment_add` | Attach a file or recording URL the user hands you to the record |

## How to Work

1. **Before creating, search.** Run `crm_lead_search` on the company or email first; update the existing lead rather
   than creating a duplicate. Check `partner_search` when the company may already be a customer.
2. **Create with what you have, never invent.** Only fields the user actually gave you. Leave expected_revenue unset
   rather than guessing.
3. **Stage moves are explicit.** Look up the current stage via `crm_lead_get`, confirm the target stage with the user
   if ambiguous, then `crm_lead_update` with the new `stage_id`.
4. **Log context as notes.** After a meaningful change, add a short note (see *Notes* below) so the reasoning lives on
   the record in Odoo, not just in this chat.
5. **Reporting.** For "how's the pipeline" questions use `crm_lead_report` grouped by stage; for "who's carrying what"
   group by owner. Present counts and expected revenue together, and say which filter window you used.

## Notes — put the reasoning on the record

Notes posted here are `mail.message` log notes authored by the acting Odoo user: visible in the record's chatter,
attributed correctly, and governed by Odoo's own access rules. If the user cannot write to a record in Odoo, the note
is refused — never route around that. Notes are not CRM-only; `note_add` takes any `model`, so the same rules apply to
`res.partner`, `project.task`, or anything else.

- "Note on the Acme lead: they want the demo pushed to next month."
- After a call or meeting, to log the outcome where the team will see it.
- To record a decision ("Ed approved the discount — log it").

1. **Resolve the record first.** Search by the name the user gave; if more than one plausible match, ask — a note on
   the wrong record is worse than no note. Never post to a record you have not resolved to a concrete `model` +
   `res_id` from a tool result.
2. **Read before writing.** `note_list` on the record so the note adds rather than repeats.
3. **Write in the user's voice, faithfully.** Tidy grammar is fine; adding interpretation is not. Short and factual:
   what happened, what was decided, by whom.
4. **Confirm with specifics.** Report back the record (model, id, name) the note landed on.
5. **Follow-ups are separate.** If the note implies a next step ("call them Friday"), say so and offer to schedule an
   activity — never create one silently.

## Activities — turn commitments into assigned work

Odoo activities are dated, assigned to-dos pinned to a record. They appear in the assignee's inbox, nag until
completed, and their completion is logged to the record's chatter. Use them for record-bound follow-ups ("call the
client Friday", "send the proposal by Tuesday", "have Ben follow up on Acme next week"). For a piece of work someone
must produce, use a project task instead — that is the `manage_work` skill.

1. **Anchor it.** Every activity lives on a record. Resolve the lead or partner first; ask if ambiguous.
2. **Check for duplicates** with `activity_list` on that record — amend rather than stack. One activity per
   commitment; do not bundle unrelated follow-ups.
3. **Explicit deadline, explicit owner.** "Next week" → propose a concrete date and confirm. Deadlines are dates the
   user agreed to, not dates you invented. The default assignee is the acting user; assigning to someone else is a
   deliberate act — name them back in your confirmation.
4. **Completion carries knowledge.** `activity_complete` with feedback, never bare — "done" tells the team nothing;
   "done — they want a September demo, budget confirmed" becomes a searchable chatter entry.
5. **Never create activities silently** from inferred commitments — surface what you found and offer.

## Rules

- A permission error from Odoo is the correct outcome for a record the user cannot write to — report it plainly.
- If the user is not linked to Odoo (Profile → Link Odoo account), stop and say so.

## Output Style

Answer with the facts from Odoo: lead ids, names, stages, owners, amounts. Link every claim to a record. When you
changed something, state exactly what changed (field, old → new) and on which lead id.
