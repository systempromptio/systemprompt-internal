# Draft Customer Email

Draft professional customer and prospect communications - deployment status, onboarding notes, incident notices, scope and pricing conversations - in systemprompt.io's voice.

## When to Use

Use this skill whenever someone at systemprompt.io needs a customer-facing email or message drafted or reviewed. It covers the recurring genres: onboarding, rollout status, release and upgrade notices, incident and bad-news delivery, sales follow-up, and meeting follow-ups. The output is always a draft for a human to review and send - never send anything yourself.

## How to Use

1. **Get the facts before writing.** Recipient(s) and their roles, the relationship temperature, what happened, what the recipient must do (if anything), and the deadline. An email without a clear purpose - inform, request, or escalate - should not be written.
2. **Pull the context from Odoo, not from memory.** Odoo is the system of record: the lead or account is a `crm.lead`, prior correspondence is on `mail.message`, and open follow-ups are `mail.activity`. Read the record before drafting, and never contradict it. If the record and the requester disagree, say so instead of picking one.
3. **Pick the genre template below** and adapt; do not invent structure per email.
4. **Write the subject line last** and make it carry the message: "Systemprompt Internal rollout - week 6: on track, one decision needed" beats "Project update".
5. **Apply the voice rules** (from `apply_brand_voice`): direct, plain, active voice, no filler. Email is the register where systemprompt.io sounds most human - contractions are fine, exclamation marks and marketing adjectives still are not.
6. **Flag anything unconfirmed** with `{{...}}` placeholders and a note to the sender. Never state dates, prices, roadmap commitments, or security claims you were not given.
7. **Log the outcome back to Odoo** once the human sends it: the sent message belongs on the record as a `mail.message`, and any follow-up as a `mail.activity` with an owner and a due date.

## Genre Templates

### Rollout / deployment status update

```
Subject: {{deployment}} - week {{n}}: {{one-line verdict}}

Hi {{name}},

Quick summary: {{one sentence - overall health and the single most important thing}}.

Done this week
- {{change shipped, stated as an outcome for their deployment}}

Coming next week
- {{planned item}}

Needs your attention
- {{decision/input}} - needed by {{date}} to keep {{milestone}} on track.

Risks we're watching
- {{risk and what we're doing about it}}

Happy to walk through any of this - otherwise we'll keep moving.

{{sender_name}}
systemprompt.io
ed@systemprompt.io | systemprompt.io
```

### Onboarding / kickoff

Order: one-line opener; what was agreed (scope, dates, environments); who's who on both sides with roles; the first three concrete steps with owners and dates - typically install the binary, point it at their Postgres and Odoo instance, and run the first governed request; the single thing needed from them first (usually an API key and network access decision); logistics (cadence, channels, support address ed@systemprompt.io).

### Release / upgrade notice

Order: what version ships and when; what changes for them, in behaviour terms; anything that requires action (config keys, migrations, env vars); what happens if they do nothing; link to the changelog. State breaking changes in the first sentence, never in a bullet halfway down.

### Acceptance / milestone sign-off request

Order: what is ready for review and where; the acceptance criteria it was tested against; the review window and deemed-acceptance date; exactly how to record approval or raise defects. One ask, one deadline.

### Incident / bad news

Order matters most here:
1. The fact, first sentence, no cushioning: "The {{component}} upgrade will miss its {{date}} target." or "Between {{start}} and {{end}}, requests through the gateway failed with {{error}}."
2. Impact, quantified: which requests, which users, what data, what it means for their go-live.
3. Cause, briefly and without blame-shifting - even when the cause is on their side, state it as a dependency fact.
4. The recovery plan: options with our recommendation, and what has already been fixed.
5. The decision needed and by when.

Never bury bad news mid-paragraph, never deliver it only in a status table, and never promise a fix you have not confirmed with whoever is doing the work.

## Rules of the Genre

- One email, one primary ask. Two asks means two emails or a call.
- Anything the recipient must act on appears in the first five lines and again as the closing line.
- Skimmable: short paragraphs, labeled sections for anything over ~10 lines.
- Names and dates over pronouns and "soon": "Ed will send the migration notes by Thursday 18 June".
- Thank-yous are specific ("thanks for turning the schema dump around in a day") or omitted.
- Never assert a security posture, certification, or data-handling guarantee in an email. Point at the approved boilerplate (`company_boilerplate`) instead.
- Sign-off: first name, then a "systemprompt.io" line, then the contact line `ed@systemprompt.io | systemprompt.io`. No inspirational quotes, no taglines in the signature, no "kindly".
- Anything commercial, legal, or security-incident related: draft, but tell the sender to route it past Ed before sending.

## Quality Gate

- [ ] Purpose (inform / request / escalate) identifiable from subject line alone
- [ ] The ask, owner, and deadline explicit (or genuinely no ask)
- [ ] Facts reconciled against the Odoo record; no unconfirmed claims; placeholders flagged to the sender
- [ ] Bad news or breaking changes, if any, in the first sentence
- [ ] Signature block correct (name, systemprompt.io, ed@systemprompt.io | systemprompt.io)
- [ ] Voice check passes (no filler, no marketing adjectives, no passive commitments)
