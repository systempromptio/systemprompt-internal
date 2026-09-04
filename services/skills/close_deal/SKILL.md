# Close Deal

Take an open lead to a decision and record it properly: won or lost, with the reason, against a real
customer — and on a win, the draft quotation that follows. Odoo is the system of record throughout.
Every write runs **as you** (the linked Odoo user), so the close carries real ownership and audit
history — no service account.

To put leads *into* the pipeline use `manage_leads`; to read what has been happening, use
`activity_report`; for a guided sweep of everything outstanding, `pending_task`.

## Ask me things like

- "We won Acme — raise the quote."
- "Mark the Northwind deal lost, they went with a competitor."
- "Close out my pipeline with me."
- "What have we quoted that nobody has come back on?"

## Prerequisite

Your platform account must be linked to your Odoo account (Profile → Link Odoo account: Odoo login +
personal API key). If a tool says you are not linked, stop and say so — never fabricate a write.

## Which mode

| Ask | Mode |
|---|---|
| "We won X" / "mark X lost" | 1 — Close one deal |
| "Close out my pipeline" / "which of mine are decided?" | 2 — Closing sweep |
| "What's quoted?" / "who owes us?" | 3 — Read the far end |

## Tools

| Tool | Use for |
|------|---------|
| `crm_lead_search` / `crm_lead_get` | Find the deal and read its current state before touching it |
| `crm_stage_list` | The real stage names — never guess a `stage_id` |
| `crm_lead_mark_won` | Close won. Runs Odoo's own win action, so stage, probability and close date move together |
| `crm_lead_mark_lost` | Close lost, with the reason. The lead is closed, not deleted |
| `crm_lead_convert_to_opportunity` | Promote a qualified enquiry before closing it, when it is still a raw lead |
| `partner_search` / `partner_create` | The customer the deal belongs to — a quotation needs a real one |
| `sale_order_create` | Raise the **draft** quotation after a win |
| `sale_order_list` / `sale_order_get` | Quotations out, and what is on one |
| `invoice_list` / `invoice_get` | What has been billed, and what is still outstanding |
| `note_add` | Record the reasoning on the lead |

---

# 1. Close one deal

1. **Read it first.** `crm_lead_get` for the current stage, owner, revenue and customer. If the user
   named the deal loosely, `crm_lead_search` and confirm *which* one before writing anything.
2. **Confirm the outcome in the user's own words.** Won and lost are not reversible in the ordinary
   course, and this is a real business record. If the user has not actually said which, ask.
3. **Check the customer.** A deal being won should sit against a real partner, not free text. If
   `crm_lead_get` shows no customer, `partner_search`; if nothing matches and the company is real,
   offer `partner_create` and link it. Say what you are about to create before you create it.
4. **Close it.**
   - Won → `crm_lead_mark_won`.
   - Lost → `crm_lead_mark_lost` with `reason` in the user's words. A reason of "lost" tells the next
     person nothing; "went with an incumbent on price" tells them everything.
   - Never write `probability` by hand to simulate either. Odoo's actions move the stage, the
     probability and the close date together; a hand-written number leaves the pipeline report
     disagreeing with the dashboard.
5. **On a win, offer the quotation.** Ask for the lines — product, quantity, and price if it differs
   from the list price. Then `sale_order_create` with `partner_id`, the lines, and `origin` set to
   the lead's name so the quote and the deal read together. It is created as a **draft**: nobody has
   sent or confirmed it, and you must say so.
6. **Log the reasoning** with `note_add` on the lead — what was decided and why.
7. **Confirm with specifics**: the lead id and name, won or lost, the reason, and the quotation id if
   one was raised.

---

# 2. Closing sweep

For "close out my pipeline with me" — the deals that look decided but are still open.

1. `crm_lead_search` with `user` set to the acting user and `open_only: true`, sorted by deadline.
2. Filter to the ones that *look* decided: expected close date already past, or no activity and no
   note for weeks. Say why each one is in the list.
3. **One at a time**, present a two-line card — where it is (stage, revenue, expected close), and
   what the last touch was — then ask one question: *"Is this won, lost, or still live?"* Wait.
   - Won or lost → mode 1, steps 3-6.
   - Still live → do not write. Offer `activity_create` for the next step instead, and move on.
4. **Close with the ledger**: every deal touched — id, name, outcome, reason, quotation raised.
   Anything left live is listed separately as still open, not silently dropped.

---

# 3. Read the far end

No writes. For "what have we quoted?" and "who owes us?".

- `sale_order_list` with `state: "draft"` — quotations sitting with a customer, nobody chasing.
  `sale_order_get` for what is actually on one.
- `invoice_list` with `unpaid_only: true` — what is billed and outstanding, biggest first. Name the
  customer, the amount outstanding and the due date.
- Report totals as Odoo returned them. Never sum across currencies without saying so.

## Rules

- **The user says won or lost. You never infer it.** A stalled deal is not a lost deal, and closing
  one that was merely quiet destroys a forecast.
- **A lost reason is required in practice.** Push back once if the user gives none; record what they
  say, not your paraphrase.
- **Quotations are drafts, always.** This skill never confirms an order and never sends one to a
  customer. Say that when you raise one, so nobody believes the customer has it.
- **Invoices are read-only here.** Raising or posting one is an accounting act with its own approval
  path. If asked, say so and stop.
- **Never invent a product, a price or a quantity.** Ask. A guessed line on a quotation is a number
  someone may quote to a customer.
- **A permission error is a correct outcome.** Odoo's record rules decide what this user may close.
  Report it plainly and stop; never route around it.
- **`note_add` is held for a second human when a non-admin calls it** (`require_approval`), so a
  non-admin's note waits at `/admin/governance/approvals` while an admin's lands directly.
- Name every record with its Odoo id so the next action is unambiguous.
- For cost and audit questions about anything this skill did, hand to `demonstrate_governance`'s
  readback.
