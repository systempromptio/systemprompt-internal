---
name: "Astound Glossary"
description: "Define and enforce consistent use of Astound Digital and digital-commerce terminology across documents and conversations"
---

# Astound Glossary

Define and enforce consistent digital-commerce and Astound Digital terminology so every document uses one term, spelled one way, meaning one thing.

## When to Use

Use this skill when drafting or reviewing any Astound content that touches commerce terminology: to look up the house definition of a term, to standardize inconsistent usage in a draft, or to add a first-use definition for a client audience. `apply_brand_voice` calls into this skill for its terminology check.

## How to Use

1. **In review mode**: sweep the document for the terms below and their variants; normalize spelling/casing to the canonical form; flag any term used with a different meaning than the house definition.
2. **In drafting mode**: use the canonical term; on first use in a client document, add the plain-language gloss from the table (one clause, not a lecture); expand every acronym at first use.
3. **For unknown terms**: if a term is not in this glossary, use the client's or vendor's own official spelling and casing, define it at first use, and suggest the requester add it to the glossary.
4. **Resolve conflicts toward the client's RFP language** in compliance sections - mirror their term, then map it once to the house term ("order management system (OMS) - the 'fulfillment hub' in your RFP").

## Core Glossary

| Canonical term | Acronym | House definition (use at first mention) |
|---|---|---|
| composable commerce | - | Building a commerce stack from independent, best-fit components (commerce engine, search, CMS, payments) connected by APIs, rather than one monolithic suite. Lowercase. |
| headless commerce | - | Architecture separating the customer-facing front end from the commerce back end, connected via APIs, so each evolves independently. "Headless" alone is acceptable after first use. |
| MACH | MACH | Microservices, API-first, Cloud-native, Headless - the architecture principles behind composable commerce. Always all-caps. |
| order management system | OMS | The system orchestrating orders after checkout: inventory visibility, routing, fulfillment, returns. |
| product information management | PIM | The system of record for product data - attributes, media, relationships - syndicated to every channel. |
| content management system | CMS | The system managing site content and experiences; in headless builds, delivers content via API. |
| enterprise resource planning | ERP | The client's finance/operations backbone; commerce integrates with it, never replaces it. |
| customer data platform | CDP | Unifies customer data from all channels into persistent profiles for activation and personalization. |
| digital experience platform | DXP | Suite combining content, personalization, and delivery for managing digital experiences. |
| B2B / B2C / D2C commerce | B2B, B2C, D2C | Business-to-business, business-to-consumer, direct-to-consumer. Always with hyphens when spelled out. |
| replatform | - | Migrating a commerce site from one platform to another. One word, no hyphen. |
| omnichannel | - | One word, no hyphen. Consistent customer experience across web, mobile, store, marketplace, and social channels. |
| average order value | AOV | Revenue divided by order count for a period. |
| conversion rate optimization | CRO | Systematic improvement of the rate at which visitors complete purchase or other goals. |
| total cost of ownership | TCO | Full lifetime cost of a solution: licenses, implementation, hosting, support, evolution. |
| user acceptance testing | UAT | Client-performed validation against acceptance criteria before go-live. |
| hypercare | - | The elevated-support period immediately after go-live, with defined duration and response times. One word. |
| go-live | - | Hyphenated as noun/adjective ("the go-live date"). |
| Agentforce | - | Salesforce's platform for building and deploying AI agents across CRM and commerce touchpoints. Capitalized; Salesforce's spelling. |
| agentic commerce | - | Commerce experiences where AI agents act on a shopper's or merchant's behalf - discovery, ordering, service - lowercase. |
| accelerator | - | Astound pre-built, reusable solution component that shortens delivery; described as licensed pre-existing IP in contracts. |

## Vendor and Platform Names

Always use the vendor's official casing and current product name: Salesforce Commerce Cloud, Adobe Commerce, SAP Commerce Cloud, commercetools (lowercase "c"), BigCommerce, Shopify. Never use deprecated names (e.g. former platform names) unless quoting the client; if the client uses one, map it once to the current name.

## Enforcement Rules

- One concept, one term per document: do not alternate "OMS" / "order hub" / "fulfillment system" for the same thing.
- Acronyms: spell out at first use with the acronym in parentheses; acronym alone thereafter. Exception: B2B, B2C, API need no expansion for commerce audiences.
- Casing is part of the term: MACH, PIM, commercetools, Agentforce - never "Mach", "Pim", "CommerceTools", "AgentForce".
- "Astound Digital" is the display name in prose; astounddigital.com in URLs; no other variants.
- Do not let marketing terms drift into contracts: in SOWs, prefer the concrete system name ("the {{oms_product}} integration") over category labels alone.

## Review Output Format

When reviewing, return a table: line/section, found term, canonical term, action taken (normalized / defined / flagged), so the author can verify each change.
