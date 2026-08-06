# Capture Knowledge

Turn raw material — a meeting transcript, a document, an email thread — into retrievable company knowledge.
Two stores, used together:

- **Knowledge bank** (`knowledge-bank` MCP): the full text, searchable across everything the company knows.
- **Odoo** (`odoo` MCP): a short summary note anchored to the record the material concerns, so anyone looking
  at that lead or customer finds it in context.

## When to Use

- "Here's the transcript from the Acme call" (pasted text or a file).
- "File this proposal / spec / research doc."
- "Save this email thread from the client."

## How to Work

1. **Classify.** source is one of `meeting-transcript`, `document`, `email`. Pick a clear title
   (e.g. "Acme discovery call — 2026-08-07"). If the material relates to a project or deal, set `project`
   to a stable slug (e.g. "acme-rollout") — it's the collection filter for later retrieval.
2. **Upload the full text**: `upload_document` (knowledge-bank) with title, source, project, and the complete
   content. Never truncate or summarize what goes into the bank — the bank holds the record, summaries go
   elsewhere. Content over ~2 MB: attach the file to the Odoo record instead (`attachment_add`) and upload a
   detailed abstract to the bank with a pointer.
3. **Summarize for the record.** If the material concerns an identifiable Odoo record (search with
   `crm_lead_search` / `partner_search` — ask if ambiguous), post a note (`note_add`) with: 3–6 bullet
   summary, decisions made, next steps, and the knowledge-bank document id ("Full transcript: kb:<id>").
4. **Schedule what was promised.** If the material contains commitments with dates, offer to create the
   follow-up activities — don't create them silently.
5. **Recordings and large media** never travel through tools as content. The blob stays in object storage or
   is uploaded via Odoo's UI; what enters the bank is the transcript, and what enters the chatter is the
   summary plus a URL-type attachment pointing at the media.

## Retrieval (the payoff)

- "What do we know about X?" → `search_project_context` (bank, cross-company) and `note_search` (Odoo,
  record-anchored) — run both, merge, cite sources by title/record.
- "What happened with Acme?" → `note_list` on the lead, plus bank search filtered to the project slug.

## Rules

- The bank starts empty and only ever contains what was deliberately captured — no auto-scraping.
- Upload verbatim content; never editorialize inside the stored document.
- One document per artifact — don't concatenate unrelated meetings into one upload.
- The Odoo note must stand alone as a useful summary even if the reader never opens the bank document.
