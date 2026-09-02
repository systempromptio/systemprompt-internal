# Brand

One front door for anything that leaves the company. Four passes, in this order — voice, then
structure, then boilerplate, then (for correspondence) the email genres. Formatting last prevents
rework; boilerplate after structure because it drops into slots the skeleton defines.

## Ask me things like

- "Rewrite this in our voice."
- "Format this as a customer document."
- "Draft the rollout status email for Acme."
- "What's our approved company description?"

## Which passes does this document need?

| Document | Passes |
|---|---|
| Product page, docs, release notes, deck, blog post | Voice |
| Quote, order form, RFP response, security questionnaire | Voice → Structure → Boilerplate |
| Rollout status, architecture readout, incident report | Voice → Structure |
| Customer or prospect email | Voice → Email |

Run the voice pass on everything. Skip a pass only when the table says to.

---

# 1. Voice

Rewrite or review content so it speaks in systemprompt.io's voice: plain, direct, technical, and
specific about what the software does. The default reader is a skeptical engineer, CTO, or CISO who
has read a hundred AI pitches this year.

1. **Identify audience and document type.** A security questionnaire tolerates more formality than a
   changelog. Voice is constant; register flexes.
2. **Read the full draft once before editing.** Note violations; do not line-edit on first read.
3. **Rewrite or annotate.** Asked to rewrite: apply the rules and return clean copy. Asked to QA:
   return violations with line references and suggested rewrites — never silently change a
   contractual document.
4. **Report what changed**: a short summary of the patterns fixed, so authors learn the voice.

## Voice principles

| Principle | Meaning |
|---|---|
| Mechanism first | Say what the software actually does, then why it matters. A reader should be able to picture the binary, the log line, or the config file. |
| Plain-spoken expert | Explain like a senior engineer in conversation: precise, no filler, no mystique. Jargon only when it earns its place — then define it once. |
| Verifiable, not boastful | Every claim is checkable against the code, the docs, or a number. No superlatives without proof. |
| Ownership is the point | We sell self-hosted AI infrastructure. Say "you run it", "your keys", "your database", "your logs" — and mean it literally. |
| Direct and accountable | Active voice, named owners, real dates. Limitations stated plainly rather than buried. |
| Neutral, not chummy | Professional and dry. No slang, no exclamation marks, no forced humour, no emoji in formal documents. |

## Do / don't

| Don't write | Write instead |
|---|---|
| "Our world-class, best-of-breed AI platform" | "One self-hosted binary that proxies every model call and writes an audit record per request" |
| "Revolutionise your organisation with agentic AI" | "Run Claude Code across a team with shared skills, per-user budgets, and a full request log" |
| "Enterprise-grade security" | The specific control: "keys stay in your environment; no request body leaves your network" |
| "It should be noted that the deployment may potentially be impacted" | "The deploy depends on {{dependency}}; if it slips, the dates move with it" |
| "Industry-leading, cutting-edge, state-of-the-art" (unproven) | A specific capability plus a specific proof point |
| "Per our previous correspondence, kindly revert at your earliest convenience" | "As discussed, could you confirm by {{date}}?" |
| Passive voice hiding the actor: "Mistakes were made in the sync" | "The Odoo sync missed activity records on `mail.activity`; it is fixed and covered by a test" |
| "Leverage synergies", "unlock value", "digital transformation journey" | Delete. Say the thing. |
| Hedging stacks: "might possibly", "somewhat unique" | One calibrated qualifier, or none |

## Mechanical rules

- Sentences average under 25 words; one idea each. Cut every "in order to", "utilize", "very", "really".
- Headings are statements or noun phrases the reader can navigate by; no clever-but-vague titles.
- Numbers are specific or absent: "p95 latency 240 ms" or nothing — never "significantly faster".
- "We" means systemprompt.io; "you/your" means the person running the software; never "the vendor"
  for ourselves and never "the end user" for them.
- Prefer product framing over agency framing: "users", "teams", "operators", "deployments" rather
  than "clients", "engagements", "transformation". Use "customer" only where a commercial
  relationship is genuinely the subject.
- Spell out an acronym at first use per document. Product names are spelled exactly: Odoo, Claude
  Code, MCP, Postgres, systemprompt.io.
- Never claim a feature that is not in the shipped code. If unsure, mark it `{{unverified}}` and flag
  it to the author.
- Avoid em-dashes in YAML-bound strings; keep punctuation simple in templates and configs.

## Terminology (applies to every pass below)

The company in prose is **systemprompt.io** — lowercase, always with the `.io`; never "Systemprompt
Inc.", never "SP". The product is **Systemprompt Internal**. The URL form is systemprompt.io; the
support contact is ed@systemprompt.io. **Odoo is the system of record** — say "Odoo" for ERP/CRM data
(leads are `crm.lead`, notes are `mail.message`, activities are `mail.activity`) and "Systemprompt
Internal" for the AI and communication layer on top of it. Never describe the two as one system. It
is a library you embed and own, never a "framework", never "a platform we host".

## Voice QA checklist

- [ ] First paragraph states what the software does, in mechanism terms
- [ ] No unproven superlatives anywhere
- [ ] Active voice in all commitments (search for "will be", "to be provided")
- [ ] No banned filler (leverage, synergy, best-of-breed, endeavor, utilize, kindly, seamless, transformation)
- [ ] Brand, product, and URL usage correct; Odoo described as the system of record
- [ ] Every factual claim traceable to code, docs, or a supplied number
- [ ] Register matches document type

---

# 2. Structure — customer documents

The final structural pass, so everything that leaves the company looks like it came from the same
company. Run it after content is settled and after the voice pass.

1. **Classify the document** (commercial / contractual / report / correspondence) — front matter
   varies by class.
2. **Apply the skeleton**: title block, document control, body, appendices. Restructure headings into
   the numbered hierarchy; never leave an orphan heading level (an H3 with no sibling).
3. **Normalize the mechanics** using the conventions below.
4. **Build the document-control table** and a table of contents (documents over 5 pages).
5. **Run the consistency sweep**: same term for the same thing throughout, same date format, same
   list punctuation, sequential numbering of figures, tables, and assumptions.
6. **Return the formatted document plus a change log** of the structural fixes made.

```
[Title page]
  Document title
  Customer name
  systemprompt.io
  Date ({{YYYY-MM-DD}})
  Version {{x.y}}
  Confidentiality marking (default: "Confidential - prepared for {{customer}}")

[Document control]               (contractual + commercial classes)
  | Version | Date | Author | Change summary |
  | Reviewer/approver table when the requester provides names |

[Table of contents]              (documents > 5 pages)

[Body]
  1. Heading level one
  1.1 Heading level two
  1.1.1 Heading level three      (maximum depth; refactor if you need 4)

[Appendices]
  Appendix A, B, C ... referenced at least once from the body

[Footer, every page]
  systemprompt.io | ed@systemprompt.io | 2026 systemprompt.io. All rights reserved.
```

| Document class | Required front matter |
|---|---|
| Contractual (order form, evaluation agreement) | Full title page, document control, parties block, signature block at end |
| Commercial (quote, RFP response, security questionnaire) | Title page, document control, confidentiality marking; follow the RFP or questionnaire's mandated format if one exists — their rules override house rules |
| Report (rollout status, architecture readout, incident report) | Compact header: title, customer, date, author, period covered |
| Correspondence | No skeleton; see section 4 |

## Formatting conventions

- **Brand furniture**: the wordmark is "systemprompt.io" in lowercase; the tagline, where a title
  page uses one, is "AI Infrastructure You Own." verbatim. Accent colour is orange `#f79938`
  (primary `#f38318`); use it for rules, table header fills, and heading accents only — never for
  body text. Body text is near-black on white; do not introduce a second accent colour.
- **Dates**: `{{D Month YYYY}}` (e.g. 11 June 2026) in prose; ISO `YYYY-MM-DD` in tables and document
  control. Never ambiguous numerics like 06/11/2026.
- **Currency**: symbol + ISO code on first use — "$25,000 USD" — then symbol alone. Placeholders as
  `{{amount}}`.
- **Tables**: header row always; one concept per column; right-align numbers; totals row bold; every
  table numbered and captioned above ("Table 3: Rollout milestones").
- **Lists**: bullets for unordered facts, numbers for sequences or anything referenced elsewhere
  ("see step 4", "Assumption A7"). Parallel grammar within a list.
- **Code, commands, and identifiers**: monospace, fenced for anything over one line. Commands shown
  exactly as run (`systemprompt infra services status`); config keys, env vars, and Odoo model names
  (`crm.lead`, `mail.message`, `mail.activity`) inline monospace. Never reflow or prettify a command
  so it stops working.
- **Emphasis**: bold for defined terms at first definition and key figures; never underline; italics
  only for document titles.
- **Headings**: sentence case, numbered, no terminal punctuation.
- **Figures**: numbered, captioned below, referenced from the body.
- **Placeholders**: always `{{snake_case_token}}`, and always listed in a "pending inputs" note — a
  placeholder reaching a customer is a defect.
- **File naming**: `systemprompt-io_{{Customer}}_{{DocType}}_{{YYYY-MM-DD}}_v{{x.y}}`.

## Structure quality gate

- [ ] Skeleton matches the document class; signature block present on contractual documents
- [ ] Heading numbering continuous, max three levels, no orphans
- [ ] All tables/figures numbered, captioned, and referenced
- [ ] Commands, config keys, and model names in monospace and copy-pasteable
- [ ] Dates, currency, and terminology consistent end to end
- [ ] Footer and confidentiality marking present; brand name and accent colour used correctly
- [ ] TOC present and matching headings (if > 5 pages)
- [ ] Pending-inputs list of remaining `{{...}}` tokens attached

---

# 3. Boilerplate

Approved company descriptions, product descriptions, and legal disclaimers. The point of boilerplate
is that it is identical everywhere — **never paraphrase it per document.**

1. **Identify which blocks the document needs** from the catalogue, by document type.
2. **Source the canonical text.** Approved boilerplate is owned by systemprompt.io (ed@systemprompt.io).
   If the requester supplies current approved text, use it verbatim. If not, insert the placeholder
   block and flag it as "requires approved text" — do not write final legal or corporate-fact
   language from memory: entity facts and version claims go stale, and legal wording carries liability.
3. **Fill only the variable slots** (`{{customer}}`, `{{date}}`, `{{document_type}}`, `{{version}}`).
   Everything else is untouchable.
4. **Match length to slot**: one-liner for footers and slide furniture, short description for cover
   pages and quote intros, long description for RFP and security-questionnaire company sections.
5. **Record which blocks and versions were used** so the requester can confirm they are current.

| Block | Used in | Notes |
|---|---|---|
| One-liner description | Email footers, docs footers, deck title slides | Single sentence: what systemprompt.io is and what it sells. Baseline framing: self-hosted AI infrastructure you run and own. |
| Tagline | Title slides, headers, footers | "AI Infrastructure You Own." Used verbatim, with the full stop. Never reworded, never extended. |
| Short description (~50-80 words) | Cover pages, quote intros, partner listings | Adds the product name, the deployment model (single self-hosted binary), and the governance surface (audit record per AI request, per-user budgets, policy chain). |
| Long description (~150-250 words) | RFP and security-questionnaire company sections | Adds architecture (Rust binary plus Postgres, Odoo as system of record, MCP servers as the tool surface), the ownership position (keys, data, and logs stay in the customer's environment), and how it differs from hosted AI platforms. |
| Product description — Systemprompt Internal | Product pages, quotes, onboarding docs | The AI and communication layer over Odoo ERP/CRM. Odoo is the system of record; this platform governs and logs every AI interaction on top of it. Do not describe the two as one system. |
| Capability statement | RFPs, technical evaluations | Structured list: governance controls, supported model providers, deployment targets, integration surface (Odoo `crm.lead` / `mail.message` / `mail.activity`, MCP servers), support model. Facts must come from approved source — placeholder until provided. |
| Confidentiality marking | Every customer-specific document | "Confidential - prepared for {{customer}} by systemprompt.io, {{date}}. Not for distribution without written consent." (Confirm against current legal standard.) |
| Quote validity disclaimer | Quotes, estimates | Pricing validity window ({{n}} days), non-binding until an order form is signed, and the basis of the estimate. Requires legal-approved wording. |
| Estimate disclaimer | Estimates | Estimate is indicative, based on stated assumptions, not a fixed-price commitment. Requires legal-approved wording. |
| Licence & self-hosting notice | Order forms, evaluation agreements | Licence terms under which the software runs on customer infrastructure, and that systemprompt.io does not receive customer prompt or response data. Legal text required. |
| Copyright line | Every published document and page | "2026 systemprompt.io. All rights reserved." The year comes from the approved block, not from today's date. |
| Legal entity & signature block | Contractual documents | Correct legal entity name — always confirm with the requester; "systemprompt.io" is the display brand, not necessarily the contracting entity. |

When approved text is not supplied, insert exactly this pattern so downstream checks catch it:

```
[[BOILERPLATE: long_company_description - INSERT APPROVED TEXT vX.Y]]
```

## Boilerplate rules

- Verbatim or placeholder — never a from-memory rewrite of legal or corporate-fact text.
- Never state customer names, revenue figures, headcounts, uptime numbers, certification claims, or
  partnership tiers unless they appear in the approved block or the requester supplies them in
  writing. Security certifications in particular are never asserted from memory.
- Disclaimers are not optional: quotes and estimates without their validity/estimate disclaimers are
  incomplete — say so.
- If two supplied blocks conflict (e.g. different licence wording), stop and ask which is current
  rather than picking one.
- [ ] Every needed block present; approved text verbatim; unapproved blocks marked and listed
- [ ] Tagline and copyright line reproduced exactly; legal entity vs brand name correct
- [ ] Version/source of each block recorded in the handover note

---

# 4. Customer email

Draft customer and prospect communications — deployment status, onboarding, incident notices, scope
and pricing. **The output is always a draft for a human to review and send. Never send anything
yourself.**

1. **Get the facts first.** Recipients and their roles, relationship temperature, what happened, what
   the recipient must do, and the deadline. An email without a clear purpose — inform, request, or
   escalate — should not be written.
2. **Pull the context from Odoo, not from memory.** The account is a `crm.lead`, prior correspondence
   is `mail.message`, open follow-ups are `mail.activity`. Read the record before drafting and never
   contradict it. If the record and the requester disagree, say so instead of picking one.
3. **Pick the genre below** and adapt; do not invent structure per email.
4. **Write the subject line last** and make it carry the message: "Systemprompt Internal rollout —
   week 6: on track, one decision needed" beats "Project update".
5. **Apply the voice rules.** Email is the register where systemprompt.io sounds most human —
   contractions are fine; exclamation marks and marketing adjectives still are not.
6. **Flag anything unconfirmed** with `{{...}}` placeholders and a note to the sender. Never state
   dates, prices, roadmap commitments, or security claims you were not given.
7. **Log the outcome back to Odoo** once the human sends it: the sent message belongs on the record as
   a `mail.message` (the `update_leads` skill's *Notes* rules), and any follow-up as a `mail.activity` with
   an owner and a due date.

### Rollout / deployment status update

```
Subject: {{deployment}} - week {{n}}: {{one-line verdict}}

Hi {{name}},

Quick summary: {{one sentence - overall health and the single most important thing}}.

Done this week
- {{change shipped, stated as an outcome for their deployment}}

Coming next week
- {{planned item}}

Needs your attention
- {{decision/input}} - needed by {{date}} to keep {{milestone}} on track.

Risks we're watching
- {{risk and what we're doing about it}}

Happy to walk through any of this - otherwise we'll keep moving.

{{sender_name}}
systemprompt.io
ed@systemprompt.io | systemprompt.io
```

### Onboarding / kickoff

One-line opener; what was agreed (scope, dates, environments); who's who on both sides with roles;
the first three concrete steps with owners and dates — typically install the binary, point it at
their Postgres and Odoo instance, and run the first governed request; the single thing needed from
them first (usually an API key and a network-access decision); logistics (cadence, channels,
ed@systemprompt.io).

### Release / upgrade notice

What version ships and when; what changes for them, in behaviour terms; anything requiring action
(config keys, migrations, env vars); what happens if they do nothing; link to the changelog. State
breaking changes in the first sentence, never in a bullet halfway down.

### Acceptance / milestone sign-off request

What is ready for review and where; the acceptance criteria it was tested against; the review window
and deemed-acceptance date; exactly how to record approval or raise defects. One ask, one deadline.

### Incident / bad news

Order matters most here:

1. The fact, first sentence, no cushioning: "The {{component}} upgrade will miss its {{date}}
   target." or "Between {{start}} and {{end}}, requests through the gateway failed with {{error}}."
2. Impact, quantified: which requests, which users, what data, what it means for their go-live.
3. Cause, briefly and without blame-shifting — even when the cause is on their side, state it as a
   dependency fact.
4. The recovery plan: options with our recommendation, and what has already been fixed.
5. The decision needed, and by when.

Never bury bad news mid-paragraph, never deliver it only in a status table, and never promise a fix
you have not confirmed with whoever is doing the work.

## Email rules

- One email, one primary ask. Two asks means two emails or a call.
- Anything the recipient must act on appears in the first five lines and again as the closing line.
- Skimmable: short paragraphs, labeled sections for anything over ~10 lines.
- Names and dates over pronouns and "soon": "Ed will send the migration notes by Thursday 18 June".
- Thank-yous are specific ("thanks for turning the schema dump around in a day") or omitted.
- Never assert a security posture, certification, or data-handling guarantee in an email. Point at
  the approved boilerplate in section 3 instead.
- Sign-off: first name, then a "systemprompt.io" line, then `ed@systemprompt.io | systemprompt.io`.
  No inspirational quotes, no taglines in the signature, no "kindly".
- Anything commercial, legal, or security-incident related: draft it, but tell the sender to route it
  past Ed before sending.

## Email quality gate

- [ ] Purpose (inform / request / escalate) identifiable from the subject line alone
- [ ] The ask, owner, and deadline explicit (or genuinely no ask)
- [ ] Facts reconciled against the Odoo record; no unconfirmed claims; placeholders flagged
- [ ] Bad news or breaking changes, if any, in the first sentence
- [ ] Signature block correct
- [ ] Voice check passes (no filler, no marketing adjectives, no passive commitments)
