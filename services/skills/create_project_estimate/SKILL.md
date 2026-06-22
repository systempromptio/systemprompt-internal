# Create Project Estimate

Build a phased, role-based effort and cost estimate for a digital-commerce engagement, with explicit assumptions and contingency.

## When to Use

Use this skill when an Astound employee needs to size an engagement: for a proposal, an RFP response, an SOW, or an internal feasibility check. The output is an estimate workbook in document form - phases, roles, hours, rates, contingency - that `create_client_proposal` and `create_statement_of_work` can consume. Do not use it to quote firm prices on its own; an estimate becomes a commercial commitment only inside a reviewed SOW or work order.

## How to Use

1. **Scope the estimate's inputs.** Ask the requester for: the scope description (or proposal/RFP), target platform(s), number and complexity of integrations, locales/sites/brands, data migration volume, and whether the client wants fixed price or T&M. Missing answers become assumptions, and assumptions widen contingency.
2. **Never invent rates.** Rates come from the requester or the current approved rate card. Use `{{rate_role}}` placeholders (e.g. `{{rate_solution_architect}}`) until real figures are supplied, and total formulas in terms of those tokens.
3. **Break work into phases, phases into work packages.** Estimate at the work-package level (e.g. "Checkout customization", "OMS integration", "PIM data model"), not the phase level - phase-level guesses hide risk.
4. **Estimate effort per role per work package** using the table format below. Use hours or days consistently throughout - state which.
5. **Apply contingency by uncertainty, not flat habit.** Use the contingency guide below and show contingency as its own line, never buried in inflated task estimates.
6. **Present three views**: effort by phase, effort by role, and cost summary. Close with the assumptions register.

## Estimate Structure

### Phase / work package / role table (example shape)

| Phase | Work package | Solution Architect | Commerce Developer | QA Engineer | Project Manager | UX Designer | Total hrs |
|---|---|---:|---:|---:|---:|---:|---:|
| Discovery | Requirements & architecture | 60 | 16 | 0 | 24 | 24 | 124 |
| Build | Catalog & PIM integration | 16 | 120 | 40 | 16 | 8 | 200 |
| Build | Checkout & payments | 12 | 100 | 36 | 12 | 16 | 176 |
| Integration | OMS integration | 20 | 90 | 30 | 12 | 0 | 152 |
| QA & Launch | UAT support, cutover, hypercare | 12 | 60 | 80 | 24 | 0 | 176 |

(The numbers above are illustrative shape only - estimate each engagement from its own scope.)

### Rate and cost table

| Role | Rate ({{currency}}/hr) | Hours | Cost |
|---|---|---:|---|
| Solution Architect | {{rate_solution_architect}} | ... | hours x rate |
| Commerce Developer | {{rate_commerce_developer}} | ... | ... |
| QA Engineer | {{rate_qa_engineer}} | ... | ... |
| Project Manager | {{rate_project_manager}} | ... | ... |
| UX Designer | {{rate_ux_designer}} | ... | ... |
| **Subtotal** | | | |
| Contingency ({{contingency_pct}}%) | | | |
| **Total** | | | |

### Contingency guide

| Situation | Suggested contingency |
|---|---|
| Well-known platform, repeat client, clear requirements | 10-15% |
| New integrations or partial requirements | 15-25% |
| New platform/version, vague scope, fixed-price ask | 25-35%, or recommend a paid discovery phase instead |

### Assumptions register

Number every assumption and tie it to the line items it protects, e.g. "A3: Client provides PIM export in agreed format - protects 'Catalog & PIM integration' (200 hrs)." A broken assumption is a change request, not absorbed effort.

## Estimating Rules

- Estimate the 80th-percentile effort, not the best case; contingency covers unknowns, not optimism.
- PM effort is typically 10-15% of delivery effort; QA 20-30% of build effort - sanity-check totals against these ratios and flag outliers.
- Anything estimated above ~120 hours as a single work package should be decomposed further.
- Always state what the estimate excludes: licenses, infrastructure/hosting, content production, third-party fees, travel.

## Quality Gate Before Delivery

- [ ] Every work package has per-role effort; totals cross-foot across all three views
- [ ] Rates are supplied values or `{{...}}` tokens, with a flagged list for the requester
- [ ] Contingency is explicit, justified against the guide, and a separate line
- [ ] Assumptions register present, numbered, and linked to line items
- [ ] Exclusions stated
