---
name: "Create RFP Response"
description: "Draft a structured RFP response for a digital-commerce engagement - solution narrative, team, timeline, commercials - in Astound's voice and format"
---

# Create RFP Response

Draft a complete, structured response to a client RFP (Request for Proposal) for a digital-commerce engagement, in Astound Digital's voice and standard format.

## When to Use

Use this skill when an Astound employee asks you to respond to an RFP, RFI, or RFQ: a prospective client has issued a formal document with requirements, evaluation criteria, and a deadline, and Astound needs a compliant, persuasive response. For an unsolicited or informal pitch, use `create_client_proposal` instead. For the contractual follow-up after a win, use `create_statement_of_work`.

## How to Use

1. **Ingest the RFP.** Read the full RFP document. Extract: issuing organization, submission deadline, required response format, mandatory sections, evaluation criteria and weighting, scope of the requested engagement, and any compliance constraints (page limits, forms, certifications).
2. **Build a requirements matrix first.** List every numbered requirement from the RFP with a planned response location. Nothing in the RFP may go unanswered - evaluators score against this list. Mark each requirement: Comply / Partially comply / Alternative proposed.
3. **Ask before inventing.** Confirm with the requester: which Astound team members or roles to name, relevant case studies or references to cite, pricing approach, and any known relationship history with the client. Never fabricate client names, certifications, or metrics. Use placeholders like `{{case_study}}` or `{{rate}}` where facts are pending.
4. **Draft the response** using the section checklist below, mirroring the RFP's own section numbering where one is mandated.
5. **Run the compliance pass.** Re-read the RFP requirements matrix against the draft. Then apply `apply_brand_voice` for tone and `format_client_document` for layout before handing the draft back.

## RFP Response Section Checklist

| # | Section | Contents |
|---|---|---|
| 1 | Cover letter | One page. Signed by engagement lead. Why Astound, in three sentences. |
| 2 | Executive summary | Client's stated problem, Astound's solution in brief, headline outcomes. No jargon. |
| 3 | Understanding of requirements | Restate the client's goals in their language. Show you read the RFP. |
| 4 | Solution narrative | Proposed architecture and approach: platform, integrations (OMS, PIM, ERP, payment, search), data migration, composable vs. monolith rationale. |
| 5 | Delivery approach | Methodology (agile cadence, sprint structure), phases, environments, QA strategy, go-live and hypercare plan. |
| 6 | Team | Named roles with short bios: Engagement Lead, Solution Architect, Commerce Developers, QA, PM, UX. Org chart of client + Astound responsibilities. |
| 7 | Timeline | Phase-level Gantt or milestone table from kickoff to hypercare exit. State assumptions that the timeline depends on. |
| 8 | Relevant experience | 2-3 case studies matching the client's industry or platform. Outcomes, not activities. |
| 9 | Commercials | Pricing model (fixed / T&M / capped T&M), phase-level figures using `{{amount}}` placeholders until pricing is approved, payment terms, what is excluded. |
| 10 | Assumptions and dependencies | Everything the price and timeline depend on: client staffing, content readiness, third-party access, decision SLAs. |
| 11 | Company information | Use `company_boilerplate` for the approved Astound Digital description, capability statement, and legal disclaimers. |
| 12 | Appendices | Requirements compliance matrix, resumes, certifications, references. |

## Writing Rules

- Lead every section with the client's outcome, then Astound's method. Evaluators skim.
- Mirror the RFP's terminology exactly when answering requirements ("the System shall...") - then translate to plain language.
- One claim, one proof: every capability statement gets a case study, metric, or named accelerator behind it, or it comes out.
- Respect hard constraints absolutely: page limits, fonts, mandatory forms. A non-compliant response is a disqualified response.
- Brand: display name is "Astound Digital"; web references use astounddigital.com.

## Quality Gate Before Delivery

- [ ] Every RFP requirement appears in the compliance matrix with a response location
- [ ] All placeholder tokens (`{{...}}`) are flagged in a summary list for the requester
- [ ] No invented clients, rates, metrics, or certifications
- [ ] Deadline, format, and page-limit constraints from the RFP are restated at the top of the draft
- [ ] Voice and formatting passes run (`apply_brand_voice`, `format_client_document`)
