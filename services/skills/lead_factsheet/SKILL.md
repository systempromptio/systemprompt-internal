# Lead Factsheet — Talk, Render, Send

Take a prospect from a conversation to a lead in Odoo, a branded factsheet written for them, and a sent email — with
every step recorded on the Odoo record. Odoo is the system of record throughout: the lead, the attachment and the
chatter entries are all created **as you** (the linked Odoo user), so ownership and audit history are real.

## Prerequisite

Your platform account must be linked to your Odoo account (Profile → Link Odoo account: Odoo login + personal API
key). Unlinked users get a clear error from every Odoo tool — link first, then retry. Factsheet rendering needs no
Odoo link; only the CRM half does.

## When to Use

- A prospect came out of a call, an event, or an inbound message, and you want them in the pipeline *and* sent
  something worth reading.
- An existing lead needs a tailored one-pager before a meeting.
- You want the standard sheet, personalised — not a new document invented from scratch.

## What a factsheet is here

A factsheet is **data**, not a document. Every sheet this instance ships is the same Handlebars template over the same
stylesheet, driven by a typed content model: pages, masthead entries, and typed blocks. The nine shipped sheets — from
the executive briefings to the technical and partner sheets — all render through that one template, which is why a
personalised sheet costs nothing: you read the closest existing sheet, change the blocks that should differ, and render.

The block types are the vocabulary. Narrative: `hero-row`, `pov`, `quote`, `sec-head`, `caps`, `personas`, `scenario`,
`why-list`, `prov`, `callout`, `cta`. Data: `ctable`, `compare`, `spec`, `cuts`, `bento4`, `ledger`, `invoice`,
`offbook`, `qbox`. Layout and framing: `two-col`, `diagram`, `flow-caption`. Call `factsheet_get` on a sheet that
already uses the block you want and copy its shape — that is faster and safer than inventing one from the schema.

Consequences worth stating, because they change how you work:

- **Never write HTML or CSS.** There is no place to put it and no need. If a sheet cannot express something, the block
  types are the limit, and the honest answer is to say so.
- **Revision is re-rendering.** There is no "edit the PDF" step. Change the document model, render again.
- **The house style is two pages** and the renderer enforces it. An overlong sheet fails with a page-count error
  naming the budget. That is a real constraint, not a glitch — shorten the copy and render again.

## Tools

| Tool | Use for |
|------|---------|
| `factsheet_list` | See which sheets ship with this instance |
| `factsheet_get` | Read a sheet's full document model — the editable form |
| `factsheet_render` | Render to a stored PDF with page previews. `sheet_id` for a shipped sheet, `doc` for an edited one |
| `crm_lead_search` | Check whether the prospect is already in the pipeline |
| `crm_lead_create` | Create the lead: name (required), partner_name, email_from, phone, description, expected_revenue |
| `crm_lead_get` | Read a lead back, including its description |
| `attachment_add` | Put the rendered PDF on the lead (`model: crm.lead`, `res_id`, `content_base64`) |
| `note_add` | Log what was sent, and why, onto the lead's chatter |
| `email_send` | Send the factsheet to the prospect |

## How to Work

Run the arc in order. Each step is confirmable, and you stop at the two points where a human decides.

1. **Capture by talking.** Ask for what you do not have; do not interrogate. A lead needs a name — everything else
   (company, email, phone, expected revenue) is worth having but optional. Never invent a field. If the prospect's
   budget did not come up, leave `expected_revenue` unset rather than guessing at it.

2. **Search before creating.** Run `crm_lead_search` on the company name and the email. If the prospect is already in
   the pipeline, work with that lead — a duplicate lead is worse than no lead. Put the new context on it with
   `note_add` instead.

3. **Create the lead.** `crm_lead_create` with only the fields you actually have. Put the conversation's substance in
   `description` — what they want, what they objected to, what happens next. This is also the raw material for the
   factsheet, so write it properly.

4. **Choose the base sheet.** `factsheet_list`, then `factsheet_get` on the closest match. Pick by audience, not by
   topic: `ceo` for an executive buyer, and whichever sheet speaks to the concern the prospect actually raised.

5. **Personalise the blocks that should differ.** Change what genuinely speaks to this prospect and leave the rest
   alone. In practice that is:
   - the **masthead** `For` entry — their name, role, or company;
   - the **hero-row** kicker and lede, so the opening addresses their situation;
   - a **caps** or **pov** block where they raised a specific objection;
   - the **cta** — the actual next step you agreed, not the generic one.

   Pages carry two layout flags worth knowing: `fill` distributes blocks over the full page height, and `dense`
   tightens the shared vertical rhythm for a page carrying a lot of blocks. Reach for `dense` before you cut content.

   Do not rewrite the claims, the sourced statistics, or the `src` citations. Those are checked copy; a factsheet that
   invents a number for one prospect is worse than a generic one. If a claim does not fit this prospect, remove the
   block rather than reword it into something unsupported.

6. **Render and show.** `factsheet_render` with the edited `doc`. It returns page-image previews and a PDF URL. Show
   the preview and say what you changed and why. **Then stop and let the user look.** Do not attach or send an
   unreviewed sheet.

7. **Iterate.** Take the correction, change the document model, render again. Repeat until the user is happy. If a
   render fails on the page budget, shorten the lede and the capability card bodies first — they carry the most slack.

8. **Attach it to the lead.** `attachment_add` with `model: crm.lead`, the lead's `res_id`, and the PDF. The document
   now lives on the record, so the next person to open the lead sees exactly what the prospect saw.

9. **Send it.** Draft the email, show it, and **get explicit approval before sending** — this is the second stop, and
   it is not optional. `email_send` posts the sent mail into the Odoo chatter with its message id, so the send is on
   the record automatically.

10. **Log the outcome.** `note_add` on the lead: which sheet went out, what was personalised, and what the agreed next
    step is. The reasoning belongs on the record, not only in this chat.

## Rules

- **Two human stops: before sending, and before attaching a sheet the user has not seen.** Everything else can flow.
- **Never invent a claim, a statistic, or a citation** to fit a prospect. Remove the block instead.
- **Never invent Odoo field values.** An absent field is honest; a guessed one corrupts the pipeline.
- **A permission error is a correct outcome.** Odoo's record rules decide what you may touch. Report the error and
  stop; never route around it.
- **One lead, one sheet, one send.** If the user wants three prospects handled, run the arc three times rather than
  batching — each sheet is personalised, and a batch cannot be reviewed properly.
