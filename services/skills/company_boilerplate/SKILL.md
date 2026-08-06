# Company Boilerplate

Insert the approved systemprompt.io company descriptions, product descriptions, and legal disclaimers into outward-facing documents - consistently, and without improvising legal language.

## When to Use

Use this skill whenever a document needs an "About systemprompt.io" section, a product description, a confidentiality marking, a licence notice, or a legal disclaimer: quotes, order forms, security questionnaires, RFP responses, documentation front matter, presentations, press-facing material. The point of boilerplate is that it is identical everywhere - never paraphrase it per document.

## How to Use

1. **Identify which blocks the document needs** from the catalog below, based on document type.
2. **Source the canonical text.** Approved boilerplate is owned by systemprompt.io (contact ed@tyingshoelaces.com). If the requester provides current approved text, use it verbatim. If not, insert the structural placeholder blocks below and flag them as "requires approved text" - do not write final legal or corporate-fact language from memory, because entity facts and version claims go stale and legal wording carries liability.
3. **Fill only the variable slots** (`{{customer}}`, `{{date}}`, `{{document_type}}`, `{{version}}`) inside approved blocks. Everything else is untouchable.
4. **Match the length to the slot**: one-liner for email footers and slide furniture, short description for cover pages and quote intros, long description for RFP and security-questionnaire company sections.
5. **Record which blocks and versions were used** so the requester can confirm they are current.

## Boilerplate Catalog

| Block | Used in | Notes |
|---|---|---|
| One-liner description | Email footers, docs footers, deck title slides | Single sentence: what systemprompt.io is and what it sells. Baseline framing: self-hosted AI infrastructure you run and own. |
| Tagline | Title slides, headers, footers | "AI Infrastructure You Own." Used verbatim, with the full stop. Never reworded, never extended. |
| Short description (~50-80 words) | Cover pages, quote intros, partner listings | Adds the product name (Systemprompt Internal), the deployment model (single self-hosted binary), and the governance surface (audit record per AI request, per-user budgets, policy chain). |
| Long description (~150-250 words) | RFP and security-questionnaire company sections | Adds architecture (Rust binary plus Postgres, Odoo as system of record, MCP servers as the tool surface), the ownership position (keys, data, and logs stay in the customer's environment), and how it differs from hosted AI platforms. |
| Product description - Systemprompt Internal | Product pages, quotes, onboarding docs | The AI and communication layer over Odoo ERP/CRM. Odoo is the system of record; this platform governs and logs every AI interaction on top of it. Do not describe the two as one system. |
| Capability statement | RFPs, technical evaluations | Structured list: governance controls, supported model providers, deployment targets, integration surface (Odoo `crm.lead` / `mail.message` / `mail.activity`, MCP servers), support model. Facts must come from approved source - placeholder until provided. |
| Confidentiality marking | Every customer-specific document | "Confidential - prepared for {{customer}} by systemprompt.io, {{date}}. Not for distribution without written consent." (Confirm wording against current legal standard.) |
| Quote validity disclaimer | Quotes, estimates | States pricing validity window ({{n}} days), non-binding status until an order form is signed, and the basis of the estimate. Requires legal-approved wording. |
| Estimate disclaimer | Estimates | Estimate is indicative, based on stated assumptions, not a fixed-price commitment. Requires legal-approved wording. |
| Licence & self-hosting notice | Order forms, evaluation agreements | States the licence terms under which the software runs on customer infrastructure, and that systemprompt.io does not receive customer prompt or response data. Legal text required. |
| Copyright line | Every published document and page | "2026 systemprompt.io. All rights reserved." The year comes from the approved block, not from today's date. |
| Legal entity & signature block | Contractual documents | Correct legal entity name - always confirm with the requester; "systemprompt.io" is the display brand, not necessarily the contracting entity. |

## Placeholder Block Shape

When approved text is not supplied, insert exactly this pattern so downstream checks catch it:

```
[[BOILERPLATE: long_company_description - INSERT APPROVED TEXT vX.Y]]
```

## Rules

- Verbatim or placeholder - never a from-memory rewrite of legal or corporate-fact text.
- Brand name in prose is "systemprompt.io" on every use, lowercase, always with the `.io`; URL form is systemprompt.io; support contact is ed@tyingshoelaces.com; no abbreviations.
- Product name is "Systemprompt Internal". Describe the software as a library customers embed and own - never a "framework", never a "platform we host".
- Never state specific customer names, revenue figures, headcounts, uptime numbers, certification claims, or partnership tiers unless they appear in the approved block or the requester supplies them in writing. Security certifications in particular are never asserted from memory.
- Disclaimers are not optional: quotes and estimates without their validity/estimate disclaimers should be flagged as incomplete.
- If two supplied blocks conflict (e.g. different licence wording), stop and ask which is current rather than picking one.

## Quality Gate

- [ ] Every needed block present (check the catalog against document type)
- [ ] Approved text verbatim; only `{{...}}` variable slots filled
- [ ] All unapproved blocks marked with the `[[BOILERPLATE: ...]]` pattern and listed for the requester
- [ ] Tagline and copyright line reproduced exactly
- [ ] Legal entity vs. brand name handled correctly on contractual documents
- [ ] Version/source of each block recorded in the handover note
