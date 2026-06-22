# Company Boilerplate

Insert the approved Astound Digital company descriptions, capability statements, and legal disclaimers into client-facing documents - consistently, and without improvising legal language.

## When to Use

Use this skill whenever a document needs an "About Astound Digital" section, a capability statement, a confidentiality marking, or a legal disclaimer: proposals, RFP responses, SOWs, work orders, presentations, press-facing material. The point of boilerplate is that it is identical everywhere - never paraphrase it per document.

## How to Use

1. **Identify which blocks the document needs** from the catalog below, based on document type.
2. **Source the canonical text.** Approved boilerplate lives with Astound's marketing and legal teams. If the requester provides current approved text, use it verbatim. If not, insert the structural placeholder blocks below and flag them as "requires approved text from marketing/legal" - do not write final legal or corporate-fact language from memory, because entity facts (office counts, headcount, partnership tiers) go stale and legal wording carries liability.
3. **Fill only the variable slots** (`{{client}}`, `{{date}}`, `{{document_type}}`) inside approved blocks. Everything else is untouchable.
4. **Match the length to the slot**: short description for cover letters and decks, long description for RFP company sections, one-liner for email footers and press notes.
5. **Record which blocks and versions were used** so the requester can confirm they are current.

## Boilerplate Catalog

| Block | Used in | Notes |
|---|---|---|
| One-liner description | Email footers, press notes, deck title slides | Single sentence: who Astound Digital is and what it does. |
| Short description (~50-80 words) | Cover letters, proposal "Why Astound" intros | Adds clients served, core services (commerce strategy, experience design, platform engineering, optimization), and global footprint. |
| Long description (~150-250 words) | RFP company-information sections | Adds history, partnership ecosystem (commerce platforms, OMS/PIM vendors), delivery model, and differentiation. |
| Capability statement | RFPs, capability decks | Structured list: service lines, platforms and certifications, industries, engagement models. Facts must come from approved source - placeholder until provided. |
| Confidentiality marking | Every client document | "Confidential - prepared for {{client}} by Astound Digital, {{date}}. Not for distribution without written consent." (Confirm wording against current legal standard.) |
| Proposal validity disclaimer | Proposals, estimates | States pricing validity window ({{n}} days), non-binding status until SOW signature, and estimate basis. Requires legal-approved wording. |
| Estimate disclaimer | Estimates | Estimate is indicative, based on stated assumptions, not a fixed-price commitment. Requires legal-approved wording. |
| IP / pre-existing materials notice | SOWs, work orders | Defers to MSA; flags Astound accelerators as licensed pre-existing IP. Legal text required. |
| Legal entity & signature block | Contractual documents | Correct legal entity name per region - always confirm with the requester; "Astound Digital" is the display brand, not necessarily the contracting entity. |

## Placeholder Block Shape

When approved text is not supplied, insert exactly this pattern so downstream checks catch it:

```
[[BOILERPLATE: long_company_description - INSERT APPROVED TEXT vX.Y FROM MARKETING]]
```

## Rules

- Verbatim or placeholder - never a from-memory rewrite of legal or corporate-fact text.
- Brand name in prose is "Astound Digital" on every use; URL form is astounddigital.com; no abbreviations.
- Never state specific client names, revenue figures, headcounts, award claims, or partnership tiers unless they appear in the approved block or the requester supplies them in writing.
- Disclaimers are not optional: proposals and estimates without their validity/estimate disclaimers should be flagged as incomplete.
- If two supplied blocks conflict (e.g. different office counts), stop and ask which is current rather than picking one.

## Quality Gate

- [ ] Every needed block present (check the catalog against document type)
- [ ] Approved text verbatim; only `{{...}}` variable slots filled
- [ ] All unapproved blocks marked with the `[[BOILERPLATE: ...]]` pattern and listed for the requester
- [ ] Legal entity vs. brand name handled correctly on contractual documents
- [ ] Version/source of each block recorded in the handover note
