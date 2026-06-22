---
name: "Create Statement of Work"
description: "Draft a full statement of work - scope, assumptions, responsibilities, change control, payment schedule - for a digital-commerce engagement"
---

# Create Statement of Work

Draft a complete Statement of Work (SOW) for a digital-commerce engagement: scope, deliverables, assumptions, responsibilities, change control, and payment schedule.

## When to Use

Use this skill when an Astound employee needs the full contractual definition of an engagement - typically after a proposal or RFP win, before delivery starts. The SOW is the document both parties will point at for the life of the project, so precision beats persuasion here. For a small, single-scope task under an existing agreement, use `create_work_order`. For the pre-sale persuasive document, use `create_client_proposal`.

## How to Use

1. **Collect the source material.** Ask for the winning proposal or RFP response, the estimate (or run `create_project_estimate` first), the MSA reference, and the client's legal entity name. The SOW must be consistent with what was sold - flag any drift you find.
2. **Interview the requester for the contentious bits**: pricing model and payment schedule, who owns content/data migration, environment and licensing responsibilities, warranty/hypercare terms, and any client-specific legal requirements.
3. **Draft using the clause skeleton below.** Every section must be filled or explicitly marked "Not applicable" with a reason - no silent omissions.
4. **Stress-test the assumptions section.** For each phase, ask: what client-side failure would blow this date or price? Each answer becomes an assumption plus a change-control trigger.
5. **Finish with the standard passes**: `apply_brand_voice`, `format_client_document`, `company_boilerplate` for entity descriptions and disclaimers. Flag explicitly that legal review is required before signature.

## SOW Clause Skeleton

```
STATEMENT OF WORK {{sow_number}}
Between {{client_legal_name}} ("Client") and Astound Digital ("Astound")
Under Master Services Agreement dated {{msa_date}}

1.  BACKGROUND & OBJECTIVES      Business context; measurable engagement goals.
2.  SCOPE OF SERVICES            Phases (e.g. Discovery, Design, Build,
                                 Integration, Data Migration, QA, Launch,
                                 Hypercare) with in-scope activities per phase.
3.  OUT OF SCOPE                 Explicit exclusions: platforms, channels,
                                 locales, integrations, content production,
                                 licensing, infrastructure costs.
4.  DELIVERABLES                 Table: deliverable, description, format,
                                 phase, acceptance owner.
5.  TIMELINE & MILESTONES        Milestone table with dependencies; statement
                                 that dates shift day-for-day with client-
                                 dependency delays.
6.  TEAM & STAFFING              Roles and allocation (Solution Architect,
                                 Commerce Developer, QA, PM, UX, Data
                                 Engineer); key-person and substitution terms.
7.  CLIENT RESPONSIBILITIES      Named product owner, decision SLA ({{n}} business
                                 days), environment/license provision, content
                                 and data readiness, UAT staffing.
8.  ASSUMPTIONS                  Numbered. Each ties to price or schedule.
9.  ACCEPTANCE PROCEDURE         Review window, acceptance criteria reference,
                                 deemed acceptance, defect severity definitions.
10. CHANGE CONTROL               Any deviation from sections 2-8 requires a
                                 written Change Request: impact assessment
                                 (scope, schedule, fees) -> mutual sign-off ->
                                 amended SOW. No verbal changes.
11. FEES & PAYMENT SCHEDULE      Model (fixed / T&M / capped T&M); table of
                                 payment events ({{amount}} per milestone or
                                 monthly); expenses; late-payment terms;
                                 invoicing details.
12. WARRANTY & HYPERCARE         Defect warranty window ({{n}} days post-launch),
                                 severity-based response times, what hypercare
                                 includes and when it ends.
13. INTELLECTUAL PROPERTY        Per MSA; note any Astound pre-existing IP or
                                 accelerators licensed, not assigned.
14. TERMINATION & SUSPENSION     Per MSA; SOW-specific wind-down terms.
15. SIGNATURES                   Authorized signatories, names, titles, dates.
```

## Drafting Rules

- Every number in the SOW (dates, fees, windows, allocations) is either supplied by the requester or a `{{...}}` placeholder. Never invent commercial or legal terms.
- Scope language is verifiable: "configure up to {{n}} payment methods" not "configure payments".
- Assumptions, client responsibilities, and change control must interlock: a failed assumption or missed responsibility routes through change control - say so in each section.
- Keep the SOW consistent with the proposal; where they must differ, list the differences for the requester rather than silently changing terms.
- Brand: "Astound Digital" as display name; astounddigital.com in any URL.

## Quality Gate Before Delivery

- [ ] All 15 sections present, filled or justified as N/A
- [ ] Out-of-scope and assumptions sections are substantive (not one-liners)
- [ ] Payment schedule events sum to the total fee
- [ ] Every `{{...}}` placeholder listed in a summary for the requester
- [ ] Explicit note attached: requires Astound legal review before signature
