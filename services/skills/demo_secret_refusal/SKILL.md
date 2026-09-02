# Demo — The Refused Secret

The `secret_scan` stage reads every tool call's arguments for credential shapes — 32 vendor patterns
(cloud keys, GitHub tokens, Stripe, Anthropic, PEM blocks, database URLs) plus a high-entropy backstop
— and refuses the call outright. It is the one stage with no exemption: an admin is refused exactly
like a user. This beat shows it firing on a real write, at $0.

## Before you start — the one rule that matters

**Never type a live-looking credential into this conversation.** The gateway scanner rescans every
turn; a real key prefix in chat blocks the *session*, not just the call. The test credentials live in
the repo fixtures and are read by the out-of-band script `demo/governance/06-secret-breach.sh` — the
audience sees the refusal without the secret ever appearing on screen. In-conversation, use the
clearly fake shapes described below; the scanner keys on shape, so they trip it just the same.

## The beat

1. **A normal write, for contrast.** `note_add` on a demo lead with an ordinary body. As a non-admin
   it is *held* (see `demo_approval_hold`); as an admin it lands. Either way it reached the stage after
   `secret_scan`.

2. **The same write with a credential in it.** `note_add` on the same lead, with a body that pastes a
   "config snippet" containing a fake key in a vendor shape — for example an AWS-style access key id
   (`AKIA` followed by sixteen upper-case letters and digits) or a GitHub-style token (`ghp_` followed
   by thirty-six letters and digits). The call is **refused**: no hold, no approver, no Odoo round
   trip. Show the reason text the tool returned — it names the pattern that matched.

3. **The email variant.** Run `send_email` with the same snippet in the body. Refused before the
   in-band confirmation is even offered: nothing was drafted, nothing waited, nothing connected.

4. **Admin included.** Have an admin run step 2. The hold they are exempt from does not matter here;
   `secret_scan` refuses them too.

5. **Read it back** (admin, `governance_readback`): two `deny` rows with `policy=secret_scan`, and the
   `allow` or `pending` row from step 1 beside them. Then the cost line — a refused call never reaches
   a provider, so the spend column for these rows is empty.

## Out of band, for the real patterns

`demo/governance/06-secret-breach.sh` posts four synthetic `PreToolUse` events straight to the
governance endpoint — an AWS key, a GitHub PAT, an RSA private key, and a clean control — with
`assert_decision` checks, so it fails loudly if the backend ever stops refusing. Run it in a terminal
beside the conversation when the audience wants to see the real vendor shapes.

## Rules

- Every claim of a refusal is read back from the audit row; show it.
- If the scanner does not fire on a fake shape, say so and switch to the script — do not "fix" the
  demo by pasting something more realistic.
- Hand off: `demo_blocked_tool` for the verdict that depends on who is asking.
