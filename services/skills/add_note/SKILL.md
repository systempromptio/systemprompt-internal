# Add Note

Capture what the user tells you into Odoo, on the record it belongs to. Notes posted here are `mail.message` log
notes authored by the acting Odoo user — visible in the record's chatter, attributed correctly, and governed by
Odoo's own access rules. If the user can't write to a record in Odoo, the note is refused; never route around that.

## When to Use

- "Note on the Acme lead: they want the demo pushed to next month."
- After a call/meeting, to log the outcome where the team will see it.
- To record a decision or permission grant against the relevant record ("Ed approved the discount — log it").

## Tools

| Tool | Use for |
|------|---------|
| `note_add` | Post the note: `model` (e.g. `crm.lead`, `res.partner`), `res_id`, `body` |
| `crm_lead_search` / `partner_search` | Resolve which record the user means before posting |
| `activity_list` | When the note implies a follow-up, check what's already scheduled |
| `note_list` | Read the record's recent chatter so the note adds, not repeats |
| `attachment_add` | When the user hands you a file (or a recording URL) for the record |

## How to Work

1. **Resolve the record first.** Search by the name the user gave; if more than one plausible match, ask — a note on
   the wrong record is worse than no note.
2. **Write the note in the user's voice, faithfully.** Tidy grammar is fine; adding interpretation is not. Keep it
   short and factual: what happened, what was decided, by whom.
3. **Confirm with specifics.** Report back the record (model, id, name) the note landed on.
4. **Follow-ups are separate.** If the note implies a next step ("call them Friday"), say so and offer to schedule it
   as an Odoo activity — don't silently create one.

## Rules

- Never post to a record you haven't resolved to a concrete `model` + `res_id` from a tool result.
- A permission error from Odoo is the correct outcome for a record the user can't write to — report it plainly.
- If the user is not linked to Odoo (Profile → Link Odoo account), stop and say so.
