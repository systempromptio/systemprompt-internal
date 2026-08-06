# Format Customer Document

Apply systemprompt.io's standard structure and formatting conventions to an outward-facing document so everything that leaves the company looks like it came from the same company.

## When to Use

Use this skill as the final structural pass on any customer-facing document: quotes, order forms, security questionnaires, RFP responses, evaluation plans, architecture readouts, rollout status reports, release notes. It handles skeleton, headings, tables, metadata, and consistency mechanics. Run it after content is settled and after `apply_brand_voice`; formatting last prevents rework.

## How to Use

1. **Classify the document** (commercial / contractual / report / correspondence) - the front matter varies slightly by class, per the table below.
2. **Apply the standard skeleton**: title block, document control, body, appendices. Restructure headings into the numbered hierarchy; never leave orphan heading levels (an H3 with no sibling).
3. **Normalize the mechanics** using the conventions list: dates, currency, tables, lists, code blocks, captions, placeholders.
4. **Build the document-control table** and table of contents (documents over 5 pages get a TOC).
5. **Run the consistency sweep**: same term for the same thing throughout, same date format, same list punctuation, sequential numbering of figures, tables, and assumptions. Product and system names are spelled exactly - systemprompt.io, Systemprompt Internal, Odoo, Claude Code, MCP, Postgres - and the terminology rules in `apply_brand_voice` apply here too.
6. **Return the formatted document plus a change log** of structural fixes made.

## Standard Skeleton

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
  systemprompt.io | ed@tyingshoelaces.com | 2026 systemprompt.io. All rights reserved.
```

| Document class | Required front matter |
|---|---|
| Contractual (order form, evaluation agreement) | Full title page, document control, parties block, signature block at end |
| Commercial (quote, RFP response, security questionnaire) | Title page, document control, confidentiality marking; follow the RFP or questionnaire's mandated format if one exists - their rules override house rules |
| Report (rollout status, architecture readout, incident report) | Compact header: title, customer, date, author, period covered |
| Correspondence | No skeleton; see `draft_client_email` |

## Formatting Conventions

- **Brand furniture**: the wordmark is "systemprompt.io" in lowercase; the tagline, where a title page uses one, is "AI Infrastructure You Own." verbatim. Accent colour is orange `#f79938` (primary `#f38318`); use it for rules, table header fills, and heading accents only - never for body text. Body text is near-black on white; do not introduce a second accent colour.
- **Dates**: write `{{D Month YYYY}}` (e.g. 11 June 2026) in prose; ISO `YYYY-MM-DD` in tables and document control. Never ambiguous numerics like 06/11/2026.
- **Currency**: symbol + ISO code on first use - "$25,000 USD" - then symbol alone. Placeholders as `{{amount}}`.
- **Tables**: header row always; one concept per column; right-align numbers; totals row bold; every table gets a number and caption above it ("Table 3: Rollout milestones").
- **Lists**: bullets for unordered facts, numbers for sequences or anything referenced elsewhere ("see step 4", "Assumption A7"). Parallel grammar within a list.
- **Code, commands, and identifiers**: monospace, in fenced blocks for anything over one line. Commands are shown exactly as run (`systemprompt infra services status`); config keys, env vars, and Odoo model names (`crm.lead`, `mail.message`, `mail.activity`) are inline monospace. Never reflow or prettify a command so it stops working.
- **Emphasis**: bold for defined terms at first definition and key figures; never underline; italics only for document titles.
- **Headings**: sentence case, numbered, no terminal punctuation.
- **Figures**: numbered and captioned below; referenced from the body text.
- **Placeholders**: always `{{snake_case_token}}`, and always listed in a "pending inputs" note to the requester - a placeholder reaching a customer is a defect.
- **File naming** (when asked to name the artifact): `systemprompt-io_{{Customer}}_{{DocType}}_{{YYYY-MM-DD}}_v{{x.y}}`.

## Quality Gate

- [ ] Skeleton matches the document class; signature block present on contractual documents
- [ ] Heading numbering continuous, max three levels, no orphans
- [ ] All tables/figures numbered, captioned, and referenced
- [ ] Commands, config keys, and model names in monospace and copy-pasteable
- [ ] Dates, currency, and terminology consistent end to end
- [ ] Footer and confidentiality marking present; brand name and accent colour used correctly
- [ ] TOC present and matching headings (if > 5 pages)
- [ ] Pending-inputs list of remaining `{{...}}` tokens attached
