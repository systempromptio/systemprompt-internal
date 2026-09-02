# Demo — The Held Call

Most policy engines answer allow or deny. This one has a third answer: **held for a named human**.
This beat produces two held calls with real tools, then resolves them two different ways in front of
the audience. Nothing here is simulated — every step is a tool call your manifest carries, and every
verdict is a row in `governance_decisions`.

## Precondition — read this first

**You must be signed in as a non-admin.** The `require_approval` stage exempts admin callers
(`exempt_scopes: [admin]`): an admin running this beat sees no hold at all and the demo looks broken.
The approver, by construction, is a *different* person from the requester. Use the demo salesperson
account from `RUN-DEMO.md`; have an admin ready in a second window at `/admin/governance/approvals`.
Check that queue is **empty** before you start — a leftover call from a rehearsal makes the beat
ambiguous.

Also confirm Odoo is linked (any read tool answers) and the queue is enabled: `services/governance/
config.yaml` lists `note_add` and `email_send` under `require_approval.patterns`.

## The beat

1. **The allow.** `crm_lead_search` for a lead to work with — say the audience's own demo lead. It
   executes at once; a `decision=allow` row lands. Name the lead and its id.

2. **The first hold — a note.** `note_add` on that lead (`model: crm.lead`, `res_id`, a short body).
   The call does not return. Explain what is happening while it waits: the governance stage answered
   `Pending`; the call is parked on an `approval_requests` row keyed by a hash of who, which server,
   which tool, which arguments — stable across retries, so a retry cannot slip a second copy through.
   After 60 seconds the server hands the wait back as an MRTR `input_required` and the client retries;
   that is the protocol working, not a timeout.

3. **The second hold — an email.** Run `send_email` to the lead's contact, logging it on the lead
   (`res_model: crm.lead`). Two stops, and the audience should see both: first the drafter confirms
   their own text in-band (round one), *then* the same `require_approval` stage parks the send for
   someone else (round two). Say plainly: the person who wrote it cannot be the person who releases it.

4. **Switch to the admin.** Open Governance → Approvals. Both calls are listed with the exact
   arguments that will run — for the email that means the full recipient list and body, not a summary.
   - **Approve the email.** It sends, and its provenance lands in the lead's chatter with its message
     id. (On a local instance without SMTP secrets it drafts and refuses at the relay — say so; the
     hold and the release are the point.)
   - **Deny the note.** It comes back refused; Odoo was never touched. Both outcomes, one queue.

5. **The race, if a second admin is on stage.** Have them click Deny on the call the first admin
   already approved. Nothing breaks and nothing is overwritten: the first decision stands, the first
   approver stays stamped, the queue simply stops listing it. A late click cannot revive an abandoned
   call and two admins cannot both own one decision.

6. **Read it back** (admin, `governance_readback`): the `pending` rows, the resumed rows with the
   approver stamped on them, and the cost line — a call waiting on a human is burning nothing.

## Rules

- Never claim a verdict you have not seen in a tool result or an audit row.
- Leave nothing parked when the demo ends: resolve or let expire (15 minutes).
- Hand off: `demo_secret_refusal` for the verdict that never reaches an approver.
