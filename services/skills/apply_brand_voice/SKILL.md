# Apply Brand Voice

Rewrite or review any piece of content so it speaks in systemprompt.io's voice: plain, direct, technical, and specific about what the software does.

## When to Use

Use this skill as the voice pass on any outward-facing content - product pages, documentation, release notes, customer emails, security questionnaires, decks, blog posts - either to rewrite a draft into the voice or to QA a near-final document against it. It pairs with `format_client_document` (structure/layout) and `company_boilerplate` (approved descriptions and disclaimers); run all three on anything that leaves the company.

## How to Use

1. **Identify the audience and document type.** A security questionnaire tolerates more formality than a changelog; a product page needs more shape than a runbook. Voice is constant, register flexes. The default reader is a skeptical engineer, CTO, or CISO who has read a hundred AI pitches this year.
2. **Read the full draft once before editing.** Note voice violations against the rules below; do not start line-editing on first read.
3. **Rewrite or annotate.** If asked to rewrite, apply the rules directly and return clean copy. If asked to QA, return a list of violations with line references and suggested rewrites - do not silently change a contractual document.
4. **Check terminology and brand usage.** Company name in prose is "systemprompt.io" (lowercase, always with the `.io`; never "Systemprompt Inc.", never "SP"); the product is "Systemprompt Internal"; the URL form is systemprompt.io; support contact is ed@systemprompt.io. Odoo is the system of record - say "Odoo" for ERP/CRM data (leads are `crm.lead`, notes are `mail.message`, activities are `mail.activity`), and "Systemprompt Internal" for the AI and communication layer on top of it. Never describe the two as one system. It is a library you embed and own, never a "framework".
5. **Report what changed**: a short summary of the patterns fixed, so authors learn the voice.

## Voice Principles

| Principle | Meaning |
|---|---|
| Mechanism first | Say what the software actually does, then why it matters. A reader should be able to picture the binary, the log line, or the config file. |
| Plain-spoken expert | Explain like a senior engineer in conversation: precise, no filler, no mystique. Jargon only when it earns its place - then define it once. |
| Verifiable, not boastful | Every claim is checkable against the code, the docs, or a number. No superlatives without proof. |
| Ownership is the point | We sell self-hosted AI infrastructure. Say "you run it", "your keys", "your database", "your logs" - and mean it literally. |
| Direct and accountable | Active voice, named owners, real dates. Limitations stated plainly rather than buried. |
| Neutral, not chummy | Professional and dry. No slang, no exclamation marks, no forced humour, no emoji in formal documents. |

## Do / Don't Rules

| Don't write | Write instead |
|---|---|
| "Our world-class, best-of-breed AI platform" | "One self-hosted binary that proxies every model call and writes an audit record per request" |
| "Revolutionise your organisation with agentic AI" | "Run Claude Code across a team with shared skills, per-user budgets, and a full request log" |
| "Enterprise-grade security" | The specific control: "keys stay in your environment; no request body leaves your network" |
| "It should be noted that the deployment may potentially be impacted" | "The deploy depends on {{dependency}}; if it slips, the dates move with it" |
| "Industry-leading, cutting-edge, state-of-the-art" (unproven) | A specific capability plus a specific proof point |
| "Per our previous correspondence, kindly revert at your earliest convenience" | "As discussed, could you confirm by {{date}}?" |
| Passive voice hiding the actor: "Mistakes were made in the sync" | "The Odoo sync missed activity records on `mail.activity`; it is fixed and covered by a test" |
| "Leverage synergies", "unlock value", "digital transformation journey" | Delete. Say the thing. |
| Hedging stacks: "might possibly", "somewhat unique" | One calibrated qualifier, or none |

## Mechanical Rules

- Sentences average under 25 words; one idea each. Cut every "in order to", "utilize", "very", "really".
- Headings are statements or noun phrases the reader can navigate by; no clever-but-vague titles.
- Numbers are specific or absent: "p95 latency 240 ms" or nothing - never "significantly faster".
- "We" means systemprompt.io; "you/your" means the person running the software; never "the vendor" for ourselves and never "the end user" for them.
- Prefer product framing over agency framing: "users", "teams", "operators", "deployments" rather than "clients", "engagements", "transformation". Use "customer" only where a commercial relationship is genuinely the subject.
- Spell out an acronym at first use per document. Product names are spelled exactly: Odoo, Claude Code, MCP, Postgres, systemprompt.io.
- Never claim a feature that is not in the shipped code. If unsure, mark it `{{unverified}}` and flag it to the author.
- Avoid em-dashes in YAML-bound strings and keep punctuation simple in templates and configs.

## QA Checklist (for review mode)

- [ ] First paragraph states what the software does, in mechanism terms
- [ ] No unproven superlatives anywhere
- [ ] Active voice in all commitments (search for "will be", "to be provided")
- [ ] No banned filler phrases (leverage, synergy, best-of-breed, endeavor, utilize, kindly, seamless, transformation)
- [ ] Brand, product, and URL usage correct throughout; Odoo described as the system of record
- [ ] Every factual claim traceable to code, docs, or a supplied number
- [ ] Register matches document type (formal for contracts and questionnaires, conversational-professional for email)
