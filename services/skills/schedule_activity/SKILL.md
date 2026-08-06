# Schedule Activity

Close the loop between what was said and what gets done. Odoo activities are dated, assigned to-dos pinned to
a record — they appear in the assignee's inbox, nag until completed, and their completion is logged to the
record's chatter. This skill creates them from commitments and marks them done with feedback.

## When to Use

- A note or meeting contains a promise: "call them Friday", "send the proposal by Tuesday".
- The user delegates: "have Ben follow up on the Acme lead next week."
- Closing out: "mark the Acme call done — they want a demo in September."

## Tools

| Tool | Use for |
|------|---------|
| `activity_create` | Schedule it: record (`model` + `res_id`), summary, deadline, assignee |
| `activity_complete` | Mark done with feedback — the feedback lands in the record's chatter |
| `activity_list` | See what's already scheduled before adding more |
| `crm_lead_search` / `partner_search` | Resolve the record the activity belongs to |

## How to Work

1. **Anchor it.** Every activity lives on a record. Resolve the lead/partner first; ask if ambiguous.
2. **Check for duplicates** with `activity_list` on that record — amend rather than stack.
3. **Explicit deadline, explicit owner.** "Next week" → propose a concrete date and confirm. Default assignee
   is the acting user; assigning to someone else is a deliberate act — name them back in your confirmation.
4. **Completion carries knowledge.** `activity_complete` with feedback, never bare — "done" tells the team
   nothing; "done — they want a September demo, budget confirmed" becomes a searchable chatter entry.
5. **Never create activities silently** from inferred commitments — surface what you found and offer.

## Rules

- One activity per commitment; don't bundle unrelated follow-ups.
- Deadlines are dates the user agreed to, not dates you invented.
