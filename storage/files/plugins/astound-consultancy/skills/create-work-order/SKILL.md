---
name: "Create Work Order"
description: "Draft a scoped work order - deliverables, milestones, acceptance criteria, billing - for a defined unit of digital-commerce delivery work"
---

# Create Work Order

Draft a scoped work order for a discrete unit of delivery work under an existing client agreement, with deliverables, milestones, acceptance criteria, and billing terms.

## When to Use

Use this skill when an Astound employee needs to formalize a specific piece of work - a feature build, an integration, a support block, a discovery sprint - under a Master Services Agreement (MSA) or existing SOW that allows work orders. A work order is narrower and faster than an SOW: one scope, one team, one billing arrangement. If the engagement needs its own assumptions, change control, and responsibility matrix, use `create_statement_of_work` instead.

## How to Use

1. **Confirm the contractual parent.** Ask the requester which MSA or SOW this work order falls under, and the work order number/identifier convention. A work order with no parent agreement is an SOW in disguise - redirect.
2. **Gather the scope facts.** What exactly is being built or done, for whom, by when, by which roles, and how it is billed (fixed fee, T&M with cap, or T&M). Use `{{rate}}` / `{{amount}}` placeholders for any commercial figure not explicitly provided; never invent rates.
3. **Draft using the skeleton below.** Keep it tight - a good work order is 2-4 pages.
4. **Make acceptance criteria testable.** Each deliverable gets criteria a client reviewer can verify objectively. "High quality code" is not acceptance criteria; "passes the agreed regression suite in the client's staging environment" is.
5. **Finish with the standard passes**: `apply_brand_voice`, `format_client_document`, and `company_boilerplate` for the signature block and legal footer.

## Work Order Skeleton

```
WORK ORDER {{wo_number}}
Under {{parent_agreement}} dated {{msa_date}}
Client: {{client_legal_name}}
Astound entity: Astound Digital
Effective date: {{start_date}}

1. PURPOSE
   One paragraph: the business outcome this work order delivers.

2. SCOPE OF WORK
   Numbered list of in-scope activities. Follow with an explicit
   OUT OF SCOPE list - this is where disputes are prevented.

3. DELIVERABLES
   | # | Deliverable | Description | Due |
   Each row is a thing the client receives, not an activity.

4. MILESTONES & SCHEDULE
   | Milestone | Target date | Dependency |
   Tie dates to dependencies ("10 business days after API access granted"),
   not just calendar dates, when client-side inputs are required.

5. ACCEPTANCE CRITERIA
   Per deliverable: objective, testable criteria; the review window
   ({{n}} business days); and the deemed-acceptance rule if the client
   does not respond within the window.

6. TEAM & ROLES
   | Role | Allocation | Rate |
   e.g. Solution Architect, Commerce Developer, QA Engineer, Project
   Manager. Rates as {{rate}} unless provided.

7. FEES & BILLING
   Pricing model, total or cap ({{amount}}), invoicing cadence,
   expenses policy, payment terms ({{payment_terms}}).

8. CLIENT RESPONSIBILITIES
   Access, environments, decision-makers, content, review SLAs.

9. ASSUMPTIONS
   Everything the estimate depends on. Each assumption that fails
   is a change request.

10. SIGNATURES
    Authorized signatories for both parties.
```

## Drafting Rules

- Scope is a fence: every in-scope line should have a matching deliverable; anything ambiguous goes in OUT OF SCOPE or ASSUMPTIONS.
- One work order, one billing model. Mixed fixed/T&M scopes belong in separate work orders.
- Dates that depend on the client are expressed relative to the dependency, not absolute.
- Do not paste legal boilerplate from memory - pull approved disclaimer text via `company_boilerplate` and flag any clause the requester must have legal review.

## Quality Gate Before Delivery

- [ ] Parent agreement identified and referenced
- [ ] Every deliverable has testable acceptance criteria and a review window
- [ ] Out-of-scope list present and non-empty
- [ ] All commercial figures are provided values or `{{...}}` placeholders, flagged for the requester
- [ ] Client responsibilities and assumptions cover every external dependency in the schedule
