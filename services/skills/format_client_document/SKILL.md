# Format Client Document

Apply Astound Digital's standard structure and formatting conventions to a client deliverable so every document leaving the firm looks like it came from the same firm.

## When to Use

Use this skill as the final structural pass on any client-facing document: proposals, RFP responses, SOWs, work orders, estimates, discovery readouts, status reports. It handles skeleton, headings, tables, metadata, and consistency mechanics. Run it after content is settled and after `apply_brand_voice`; formatting last prevents rework.

## How to Use

1. **Classify the document** (proposal / contractual / report / correspondence) - the front-matter and tone of the skeleton vary slightly by class, per the table below.
2. **Apply the standard skeleton**: title block, document control, body, appendices. Restructure headings into the numbered hierarchy; never leave orphan heading levels (an H3 with no sibling).
3. **Normalize the mechanics** using the conventions list: dates, currency, tables, lists, captions, placeholders.
4. **Build the document-control table** and table of contents (documents over 5 pages get a TOC).
5. **Run the consistency sweep**: same term for the same thing throughout (cross-check `astound_glossary`), same date format, same list punctuation, sequential numbering of figures, tables, and assumptions.
6. **Return the formatted document plus a change log** of structural fixes made.

## Standard Skeleton

```
[Title page]
  Document title
  Client name
  Astound Digital
  Date ({{YYYY-MM-DD}})
  Version {{x.y}}
  Confidentiality marking (default: "Confidential - prepared for {{client}}")

[Document control]               (contractual + proposal classes)
  | Version | Date | Author | Change summary |
  | Reviewer/approver table when the requester provides names |

[Table of contents]              (documents > 5 pages)

[Body]
  1. Heading level one
  1.1 Heading level two
  1.1.1 Heading level three      (maximum depth; refactor if you need 4)

[Appendices]
  Appendix A, B, C ... referenced at least once from the body
```

| Document class | Required front matter |
|---|---|
| Contractual (SOW, work order) | Full title page, document control, parties block, signature block at end |
| Proposal / RFP response | Title page, document control, confidentiality marking; follow RFP-mandated format if one exists - RFP rules override house rules |
| Report (status, readout) | Compact header: title, client, date, author, period covered |
| Correspondence | No skeleton; see `draft_client_email` |

## Formatting Conventions

- **Dates**: write `{{D Month YYYY}}` (e.g. 11 June 2026) in prose; ISO `YYYY-MM-DD` in tables and document control. Never ambiguous numerics like 06/11/2026.
- **Currency**: symbol + ISO code on first use - "$250,000 USD" - then symbol alone. Placeholders as `{{amount}}`.
- **Tables**: header row always; one concept per column; right-align numbers; totals row bold; every table gets a number and caption above it ("Table 3: Milestone schedule").
- **Lists**: bullets for unordered facts, numbers for sequences or anything referenced elsewhere ("see step 4", "Assumption A7"). Parallel grammar within a list.
- **Emphasis**: bold for defined terms at first definition and key figures; never underline; italics only for document titles.
- **Headings**: sentence case, numbered, no terminal punctuation.
- **Figures**: numbered and captioned below; referenced from the body text.
- **Placeholders**: always `{{snake_case_token}}`, and always listed in a "pending inputs" note to the requester - a placeholder reaching a client is a defect.
- **File naming** (when asked to name the artifact): `AstoundDigital_{{Client}}_{{DocType}}_{{YYYY-MM-DD}}_v{{x.y}}`.

## Quality Gate

- [ ] Skeleton matches the document class; signature block present on contractual documents
- [ ] Heading numbering continuous, max three levels, no orphans
- [ ] All tables/figures numbered, captioned, and referenced
- [ ] Dates, currency, and terminology consistent end to end
- [ ] TOC present and matching headings (if > 5 pages)
- [ ] Pending-inputs list of remaining `{{...}}` tokens attached
