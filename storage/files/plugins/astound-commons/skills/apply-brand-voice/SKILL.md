---
name: "Apply Brand Voice"
description: "Rewrite or QA content against Astound Digital's voice and tone rules - confident, plain-spoken, client-outcome led"
---

# Apply Brand Voice

Rewrite or review any piece of content so it speaks in Astound Digital's voice: confident, plain-spoken, evidence-backed, and led by client outcomes.

## When to Use

Use this skill as the voice pass on any outward-facing content - proposals, RFP responses, SOWs, emails, decks, web copy - either to rewrite a draft into the voice or to QA a near-final document against it. It pairs with `format_client_document` (structure/layout) and `astound_glossary` (terminology); run all three on client deliverables.

## How to Use

1. **Identify the audience and document type.** A SOW tolerates more formality than a kickoff email; a proposal needs more energy than a status report. Voice is constant, register flexes.
2. **Read the full draft once before editing.** Note voice violations against the rules below; do not start line-editing on first read.
3. **Rewrite or annotate.** If asked to rewrite, apply the rules directly and return clean copy. If asked to QA, return a list of violations with line references and suggested rewrites - do not silently change a contractual document.
4. **Run the terminology check** from `astound_glossary` for commerce terms, and confirm brand usage: display name "Astound Digital" (never "Astound Digital Inc." in prose, never just "AD"), URLs as astounddigital.com.
5. **Report what changed**: a short summary of the patterns fixed, so authors learn the voice.

## Voice Principles

| Principle | Meaning |
|---|---|
| Client-outcome led | Open with what the client gets, then how Astound does it. |
| Plain-spoken expert | Explain like a senior consultant in conversation: precise, no filler, no mystique. Jargon only when it earns its place - then define it once. |
| Confident, not boastful | State capability as fact with evidence. No superlatives without proof. |
| Direct and accountable | Active voice, named owners, real dates. We say what we will do and who does it. |
| Warm, not chummy | Professional warmth. No slang, no exclamation marks in formal documents, no forced humor. |

## Do / Don't Rules

| Don't write | Write instead |
|---|---|
| "We are pleased to leverage our world-class, best-of-breed solutions" | "We will build this on {{platform}}, which we've delivered for {{n}} similar retailers" |
| "It should be noted that the timeline may potentially be impacted" | "The timeline depends on {{dependency}}; if it slips, the dates move with it" |
| "Our team will endeavor to ensure synergies are actioned" | "{{name}} owns this and will deliver it by {{date}}" |
| "Industry-leading, cutting-edge, state-of-the-art" (unproven) | A specific capability plus a specific proof point |
| "Per our previous correspondence, kindly revert at your earliest convenience" | "As discussed, could you confirm by {{date}}?" |
| Passive voice hiding the actor: "Mistakes were made in the integration" | "The integration missed the order-status webhook; we've fixed it and added a regression test" |
| Hedging stacks: "might possibly", "somewhat unique" | One calibrated qualifier, or none |

## Mechanical Rules

- Sentences average under 25 words; one idea each. Cut every "in order to", "utilize", "very", "really".
- Headings are statements or noun phrases the reader can navigate by; no clever-but-vague titles in deliverables.
- Numbers are specific or absent: "reduced page load 38%" or nothing - never "significantly improved".
- "We" means Astound Digital; "you/your" means the client; never "the vendor"/"the consultant" for ourselves.
- Spell out an acronym at first use per document (see `astound_glossary`).
- Avoid em-dashes in YAML-bound strings and keep punctuation simple in templates and configs.

## QA Checklist (for review mode)

- [ ] First paragraph of the document states a client outcome
- [ ] No unproven superlatives anywhere
- [ ] Active voice in all commitments (search for "will be", "to be provided")
- [ ] No banned filler phrases (leverage, synergy, best-of-breed, endeavor, utilize, kindly)
- [ ] Brand name and URL usage correct throughout
- [ ] Register matches document type (formal for SOW/RFP, conversational-professional for email)
