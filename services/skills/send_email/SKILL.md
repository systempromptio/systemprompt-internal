# Send Email

Send an email from the conversation. This is the one tool on the instance that reaches outside the
company, and it is built so that it cannot act on a single person's say-so: you confirm your own draft,
then a **second** human releases it. Both stops are real controls, not ceremony, and both leave an
audit row.

## Ask me things like

- "Email Acme the revised timeline."
- "Send the factsheet link to jane@example.com and log it on her lead."
- "Reply to the prospect from yesterday's call."

## The tool

One tool, `email_send` (on the `email` server), with these arguments:

| Argument | Required | Meaning |
|----------|----------|---------|
| `to` | yes | Recipient addresses, one or more |
| `subject` | yes | Subject line |
| `body` | yes | Plain-text body |
| `reply_to` | no | Where replies go; never becomes the `From:` |
| `res_model` + `res_id` | no | The Odoo record to log the sent mail onto — e.g. `crm.lead` + the lead id. Both or neither |

If the mail concerns a lead or contact, pass the record: the send then appears in that record's chatter
with its message id, so the team sees it where they work. This skill cannot look records up — if you do
not know the id, the `update_leads` and `lead_factsheet` skills resolve it and call `email_send` themselves.

## How to work

1. **Get the facts.** Recipient, what the mail is for, and any specifics (dates, amounts, names). Ask
   for what is missing; never invent a detail to fill a gap.
2. **Draft in the house voice.** Apply the `brand` skill's email pass: its genre templates (status,
   onboarding, release notice, sign-off) and its voice rules. Show the full draft — recipients, subject,
   body — in the conversation before calling anything.
3. **Call `email_send`.** The call runs in two rounds, and you must narrate them honestly:
   - **Round one — your confirmation.** The tool returns a preview artifact and an `approve_send`
     elicitation asking you to set `confirm`. This is the drafter checking their own text. Nothing has
     been sent.
   - **Round two — someone else's approval.** The `require_approval` governance stage parks the call
     for an admin at `/admin/governance/approvals`, where the approver sees the exact recipients and
     body that will go on the wire. The conversation receives `input_required` (MRTR) while it waits;
     the client retries automatically. Say "waiting on an approver" — never "failed" and never "sent".
4. **Report the outcome, whichever it is.** Exactly one of:
   - **Sent** — quote the message id; if a record was given, say the chatter entry landed.
   - **Denied** by the approver — say so plainly; nothing reached the relay.
   - **Expired** — the hold lasts 15 minutes (`expiry_seconds: 900`); an unanswered call lapses.
     Offer to resend, which starts both rounds again.
   - **Refused by the secret scanner** — a credential-shaped string in the body is denied before the
     hold and before any SMTP connection, at $0. Remove it and redraft.

## What you must know

- **An admin's own send is never held.** `exempt_scopes: [admin]` means round two does not exist for an
  admin caller — their in-band confirmation is the only stop. Tell an admin that when they send.
- **Local instances without `smtp_*` secrets draft and then refuse to send.** That is the expected
  local shape, not an error to work around; the approval round-trip is the part that matters.
- **There is no draft/send pair and no bypass.** A client that does not implement MRTR never gets past
  round one. Do not look for another way to send.
- **Never paste a credential into a draft.** The gateway scanner rescans every turn; a live-looking key
  in the conversation blocks the session, not just the send.

## Output style

Show the draft as it will send. After the call, one line per outcome with the id or the reason. If the
mail was logged to Odoo, name the record (model, id).
