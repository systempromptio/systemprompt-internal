---
name: "Create Client Proposal"
description: "Draft a persuasive client proposal - situation, approach, value, next steps - for a digital-commerce engagement in Astound's voice"
---

# Create Client Proposal

Draft a persuasive, client-facing proposal for a digital-commerce engagement: situation, approach, value, and next steps, in Astound Digital's voice.

## When to Use

Use this skill when an Astound employee wants to pitch work to a client or prospect outside a formal RFP process - after a discovery call, a workshop, or an inbound request. The proposal's job is to win a decision, not to define a contract. For a formal RFP, use `create_rfp_response` (compliance-driven); once the proposal is accepted, use `create_statement_of_work` (contract-driven).

## How to Use

1. **Capture the client's situation in their own words.** Ask the requester for call notes, the client's stated pains, who the decision-makers are, the budget signal if any, and the deadline driving them. The strongest proposals quote the client's framing back to them.
2. **Decide the single core argument.** Every proposal answers one question: "Why Astound, why this approach, why now?" Write that answer as one sentence before drafting anything - every section must serve it.
3. **Size the work.** Run `create_project_estimate` (or request an existing estimate) so the commercial section is grounded. Use `{{amount}}` / `{{rate}}` placeholders if pricing is not yet approved.
4. **Draft using the structure below.** Keep it short: 6-12 pages. A proposal the buyer can read in one sitting beats a comprehensive one they skim.
5. **Make next steps frictionless.** End with a dated, specific path to "yes" - not "we look forward to hearing from you".
6. **Finish with the standard passes**: `apply_brand_voice`, `format_client_document`, and `company_boilerplate` for the about-us section.

## Proposal Structure

| Section | Purpose | Rules |
|---|---|---|
| 1. Executive summary | The whole argument on one page | Situation in one paragraph, recommendation in one, value in one. Write it last. |
| 2. Your situation | Prove Astound listened | Client's goals, constraints, and pains in their language. No Astound content here. |
| 3. What's at stake | Create urgency honestly | Cost of inaction or delay, framed from the client's metrics (conversion, AOV, time-to-market, TCO). No fear-mongering. |
| 4. Our recommended approach | The how | Phased approach with rationale: why this platform/architecture (composable, headless, replatform vs. optimize), why this sequence, what the client sees at each phase end. |
| 5. Why Astound | Differentiate with evidence | Relevant case studies (2 max), accelerators, partnerships, team strengths. Claims need proof or they come out. |
| 6. Team | Faces build trust | Key roles: Engagement Lead, Solution Architect, Commerce Developer leads, PM. Short, relevant bios. |
| 7. Timeline | Make it feel real | Milestone view from signature to first value. Highlight the earliest visible win. |
| 8. Investment | Frame cost as value | Phase-level figures ({{amount}}), pricing model, what is included/excluded. Place after value, never before. |
| 9. Next steps | Remove friction | Concrete: "Sign by {{date}}, kickoff {{date}}, discovery readout {{date}}." Offer a decision call. |

## Persuasion Rules

- Sell outcomes, not effort: "reduce checkout abandonment" beats "implement checkout optimizations".
- Use "you/your" at least twice as often as "we/our" - count it in section 2 especially.
- One idea per paragraph; one proof per claim; cut any sentence that could appear in a competitor's proposal unchanged.
- Never disparage the incumbent vendor or the client's past decisions - frame the past as a reasonable choice whose context has changed.
- No invented metrics, clients, or discounts. Placeholders plus a flag list for anything unconfirmed.
- Brand: "Astound Digital" display name; astounddigital.com for URLs.

## Quality Gate Before Delivery

- [ ] Core argument stated in one sentence at the top of the draft (internal note for the requester)
- [ ] Executive summary works standalone - test by reading it without the rest
- [ ] Section 2 contains zero Astound self-promotion
- [ ] Every claim in section 5 carries evidence
- [ ] Investment section uses approved figures or flagged `{{...}}` placeholders
- [ ] Next steps include at least one specific date or decision ask
