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
| `attachment_list` | See what documents are already on the record |

## How to Work

1. **Before creating, search.** Run `crm_lead_search` on the company or email first; update the existing lead rather
   than creating a duplicate. Check `partner_search` when the company may already be a customer.
2. **Create with what you have, never invent.** Only fields the user actually gave you. Leave expected_revenue unset
   rather than guessing.
3. **Stage moves are explicit.** Look up the current stage via `crm_lead_get`, confirm the target stage with the user
   if ambiguous, then `crm_lead_update` with the new `stage_id`.
4. **Log context as notes.** After a meaningful change, add a short note with the `add_note` skill (`note_add` tool on
   `crm.lead`) so the reasoning lives on the record in Odoo, not just in this chat.
5. **Reporting.** For "how's the pipeline" questions use `crm_lead_report` grouped by stage; for "who's carrying what"
   group by owner. Present counts and expected revenue together, and say which filter window you used.

## Output Style

Answer with the facts from Odoo: lead ids, names, stages, owners, amounts. Link every claim to a record. When you
changed something, state exactly what changed (field, old → new) and on which lead id.
