# Demo 3 — Follow-Up Orchestrator

The "sales engagement" use case, and the first demo step that **writes**. Five mutations land in one
conversation — four in Odoo, one leaving the building as real email — and every one of them executes
as the signed-in user, so Odoo's record rules and Odoo's own audit log apply. There is no integration
user with god rights to hide behind.

This is also the step where the trust layer stops being a slide. The email is held for a second human
before it sends, and a second attempt is killed outright by the secret scanner. Both happen on stage,
in front of the audience, on the real system.

## When to Use

- Step 4 of the enterprise demo sequence in DEMO.md runs after this; run this after `demo_account_360`,
  acting on the next step that brief suggested.
- Any "follow up with <lead>" request where the governed-writes framing helps.

## Before you start: run this as a salesperson, not as an admin

Sign in as `ed+notadmin@systemprompt.io` (RUN-DEMO.md has the cast list). The approval stage carries
`exempt_scopes: [admin]`, so **an admin caller is never held** — the approver and the requester must
be different people or it is a rubber stamp, not a control. Demoing this as an admin shows no hold at
all and looks like the control is broken. It isn't; you're exempt.

## Script

Confirm the target lead (Odoo id from step 2) and the follow-up intent, then execute in order,
narrating each write with the id Odoo returns:

1. **Task** — `activity_create` on the lead: a follow-up activity, assigned to the acting user, due in
   a few days.
2. **Meeting** — `calendar_event_create`: a short call linked to the lead, with a sensible near-future
   slot (confirm the time with the user first).
3. **Note** — draft the follow-up message applying the house voice (the `brand` skill's voice pass defines
   it — apply its rules; do not restate them), then log it on the lead with `note_add`.
4. **Email** — `email_send` the follow-up to the lead's own address. This is the centrepiece; work it
   in three beats, below.
5. **Team visibility** — `channel_post` to the sales channel (find it with `channel_list`): one line —
   who, which lead, what was scheduled.

### Beat 1 — the drafter confirms their own text

The tool renders the drafted mail back as a card and asks the salesperson to confirm it before
anything is sent. Read the draft out. Change something and re-confirm if the audience wants to see it
re-render.

Say what this layer is, and no more: the person who wrote it checking their own work. It is not
oversight — nobody else has seen it yet. That is the next beat.

### Beat 2 — a second human releases it

The confirmed call does not send. It parks, and `/admin/governance/approvals` now holds a row naming
the tool, the caller, the rule that stopped it, and the exact recipients and body that will go on the
wire. In the DEMO.md running order you leave it parked here and release it in Act II as the admin
(step 3b), alongside the other held calls. The mail sends on approval.

If a second admin is on hand, run the race: have them click Deny on the call the first admin just
approved. Nothing is overwritten. The first decision stands, the original approver stays stamped in
the audit row, and the queue stops listing it — `resolve` returns `Ok(None)` for a row already decided
or expired, and the console treats that as an ordinary outcome, not an error. A late click cannot
revive an abandoned call, and two admins cannot both own one decision.

The point to land: the drafter cannot approve their own send, and the approver reviews the real
payload rather than the agent's description of it. Separation of duties is enforced by the policy
engine, not promised by the agent — and the salesperson could not have talked their way past it,
because the model was never asked. Note the direction of the exemption: `exempt_scopes: [admin]`
exempts an admin *requester* from being held; it does not stop an admin from *approving*.

Then open the lead in Odoo (`:8070`) and show the chatter entry the send just wrote — logged as an
email, under the salesperson's own name, in the same call. The CRM is still the system of record. That
is the whole difference from a bolt-on that emails from a shared mailbox and leaves the record blank.

### Beat 3 — the send that never leaves

Now try to send a follow-up whose body pastes in a config snippet containing a live-looking key (an
`sk-ant-…` string). `secret_scan` denies it on the arguments, by pattern name, **before** SMTP is
touched and before either approval layer is reached.

Show the error verbatim, then `infra logs audit <id>`: `decision=deny`, the pattern that matched, and
**$0** — enforcement fires ahead of model spend and ahead of the outside world. Nothing was sent,
nothing was held for a human, and nobody had to notice.

### The record-rules denial (optional, still the cleanest identity proof)

Target a record `e2e-sales` cannot write to: Odoo itself refuses. Show the error verbatim, then the
same call succeeding as the admin. Authorization is enforced by the system of record per user, not
promised by the agent.

## Cost readback

```bash
systemprompt infra logs trace list --limit 10       # one trace per mutation, in order
systemprompt infra logs request list --limit 5
```

State: five writes, one of them held for a named human and one refused outright, total cost $X, each
with a trace id — and each also visible in Odoo's own chatter under the acting user's name.

## Rules

- Never invent times, owners, recipients, or channels — confirm ambiguous choices with the user before
  writing, and **always** confirm the recipient address before sending.
- Report every returned id; if a write fails, stop, show the error, and do not improvise around it.
- If the approval queue is empty when you expect a hold, you are signed in as an admin. Stop and switch
  users rather than narrating around it.
- Hand off: end by offering step 4, `demo_governed_operations` (admin-only — switch to the admin user).
