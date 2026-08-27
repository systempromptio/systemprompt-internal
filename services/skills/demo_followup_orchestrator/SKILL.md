# Demo 3 — Follow-Up Orchestrator

The "sales engagement" use case, and the first demo step that **writes**. Four mutations land in Odoo in
one conversation — and every one of them executes as the signed-in user, so Odoo's record rules and
Odoo's own audit log apply. There is no integration user with god rights to hide behind.

## When to Use

- Step 4 of the enterprise demo sequence in DEMO.md runs after this; run this after `demo_account_360`,
  acting on the next step that brief suggested.
- Any "follow up with <lead>" request where the governed-writes framing helps.

## Script

Confirm the target lead (Odoo id from step 2) and the follow-up intent, then execute in order,
narrating each write with the id Odoo returns:

1. **Task** — `activity_create` on the lead: a follow-up activity, assigned to the acting user, due in
   a few days.
2. **Meeting** — `calendar_event_create`: a short call linked to the lead, with a sensible near-future
   slot (confirm the time with the user first).
3. **Note** — draft the follow-up message applying the house voice (the `apply_brand_voice` skill defines
   it — apply its rules; do not restate them), then log it on the lead with `note_add`. In this demo the
   note on the record *is* the outbound draft — nothing is emailed anywhere.
4. **Team visibility** — `channel_post` to the sales channel (find it with `channel_list`): one line —
   who, which lead, what was scheduled.

### The denial beat (optional but the strongest moment)

Run this step signed in as `e2e-sales@systemprompt.local` and target a record that user cannot write to:
Odoo itself refuses. Show the error verbatim, then the same call succeeding as the admin. The point:
authorization is enforced by the system of record per user, not promised by the agent.

## Cost readback

```bash
systemprompt infra logs trace list --limit 10       # one trace per mutation, in order
systemprompt infra logs request list --limit 5
```

State: four writes, total cost $X, each with a trace id — and each also visible in Odoo's own chatter
under the acting user's name (open the lead in Odoo at :8070 if the audience wants proof).

## Rules

- Never invent times, owners, or channels — confirm ambiguous choices with the user before writing.
- Report every returned id; if a write fails, stop, show the error, and do not improvise around it.
- Hand off: end by offering step 4, `demo_governed_operations` (admin-only — switch to the admin user).
