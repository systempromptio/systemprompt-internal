---
name: "Draft Client Email"
description: "Draft professional client communications - status updates, kickoffs, escalations - in Astound Digital's voice"
---

# Draft Client Email

Draft professional client communications - status updates, kickoff notes, escalations, scope conversations - in Astound Digital's voice.

## When to Use

Use this skill whenever an Astound employee needs a client-facing email or message drafted or reviewed. It covers the recurring genres of consulting delivery: weekly status, kickoff, milestone/acceptance requests, schedule or scope escalations, bad-news delivery, and meeting follow-ups. The output is always a draft for the employee to review and send - never send anything yourself.

## How to Use

1. **Get the facts before writing.** Recipient(s) and their roles, the relationship temperature, what happened, what the client must do (if anything), and the deadline. An email without a clear purpose - inform, request, or escalate - should not be written.
2. **Pick the genre template below** and adapt; do not invent structure per email.
3. **Write the subject line last** and make it carry the message: "Acme replatform - week 14 status: on track, one decision needed" beats "Project update".
4. **Apply the voice rules** (from `apply_brand_voice`): direct, warm, active voice, no filler. Emails are the register where Astound sounds most human - contractions are fine, exclamation marks still are not.
5. **Flag anything unconfirmed** with `{{...}}` placeholders and a note to the sender. Never state dates, costs, or commitments you were not given.

## Genre Templates

### Weekly status update

```
Subject: {{project}} - week {{n}} status: {{one-line verdict}}

Hi {{name}},

Quick summary: {{one sentence - overall health and the single most important thing}}.

Done this week
- {{accomplishment with outcome, not activity}}

Coming next week
- {{planned item}}

Needs your attention
- {{decision/input}} - needed by {{date}} to keep {{milestone}} on track.

Risks we're watching
- {{risk and what we're doing about it}}

Happy to walk through any of this - otherwise we'll keep moving.

{{sender_name}}
Astound Digital
```

### Kickoff

Order: warm one-line opener; what was agreed (scope, dates, team); who's who on both sides with roles; the first three concrete steps with owners and dates; the single thing needed from the client first; logistics (cadence, channels, tools).

### Acceptance / milestone sign-off request

Order: what is ready for review and where; the acceptance criteria it was tested against; the review window and deemed-acceptance date from the SOW; exactly how to record approval or raise defects. One ask, one deadline.

### Escalation / bad news

Order matters most here:
1. The fact, first sentence, no cushioning: "The {{integration}} milestone will miss its {{date}} target."
2. Impact, quantified: what it means for go-live, cost, or scope.
3. Cause, briefly and without blame-shifting - even when the cause is client-side, state it as a dependency fact.
4. The recovery plan: options with Astound's recommendation.
5. The decision needed and by when.

Never bury bad news mid-paragraph, never deliver it only in a status table, and never promise a fix you have not confirmed with delivery.

## Rules of the Genre

- One email, one primary ask. Two asks means two emails or a meeting.
- Anything the client must act on appears in the first five lines and again as the closing line.
- Skimmable: short paragraphs, labeled sections for anything over ~10 lines.
- Names and dates over pronouns and "soon": "Maria will send the test plan by Thursday 18 June".
- Thank-yous are specific ("thanks for turning the catalog export around in a day") or omitted.
- Sign-off: first name, then "Astound Digital" line. No inspirational quotes, no "kindly".
- Escalations above project level (commercial disputes, legal, security incidents): draft, but tell the sender to route through the engagement lead before sending.

## Quality Gate

- [ ] Purpose (inform / request / escalate) identifiable from subject line alone
- [ ] The ask, owner, and deadline explicit (or genuinely no ask)
- [ ] No unconfirmed facts; placeholders flagged to the sender
- [ ] Bad news, if any, in the first sentence
- [ ] Voice check passes (no filler, no passive commitments)
