# Demonstrate Governance

Most policy engines answer allow or deny. This one has a third answer: **held for a named human**.
This skill runs any of three real governance scenarios with real tool calls, then reads back the
audited decision trail — latency, decision, cost — for exactly what just happened. One flow: pick a
scenario, run it, read it back.

## Ask me things like

- "Show me a call get held for approval."
- "Show a secret get refused."
- "Show a destructive tool get blocked."
- "What just happened, and what did it cost?"

## The pipeline

Every tool call runs a synchronous five-stage check before it executes (config in
`services/governance/config.yaml`): **scope check → secret scan → blocklist → rate limit → require
approval**. Each decision is written to `governance_decisions` with the tool, the agent, the policy,
and the reason.

| Stage | Policy id | What it does | Admin exempt? |
|-------|-----------|---------------|----------------|
| Scope check | `scope_check` | Refuses non-admin scope calling `mcp__systemprompt__*` tools | Yes |
| Secret scan | `secret_scan` | Refuses plaintext credentials in any tool input (35+ patterns), any scope | **No** |
| Blocklist | `tool_blocklist` | Refuses destructive tool names (`delete`, `drop`, `destroy`) for user/non-admin scope | Yes |
| Rate limit | `rate_limit` | Refuses more than 300 calls per 60s for one identity | No |
| Require approval | `require_approval` | Parks the call — the third verdict, `Pending` — for a named human to approve or deny | Yes |

Scope is derived from the **caller's live DB roles**, not the `agent_id` in the payload. To
demonstrate a `scope_check`, `tool_blocklist`, or `require_approval` verdict you need a **non-admin**
caller — an admin is exempt from all three and the demo looks broken. `secret_scan` is the one stage
with no exemption: it fires for every identity, admin included.

**Precondition — check first.** All five stages are enabled in this installation. Confirm before
running anything: a normal in-scope tool call should land a `decision=allow` row with a real policy
id. A row reading `policy=governance_disabled` means the stages were switched off; re-enable them
and restart before demoing. Never change state by deleting the config file — a missing file falls
back to defaults, which is the original four stages **enabled** (not `require_approval`, which is
opt-in by design).

## Pick a scenario

| Scenario | Verdict shown | Run as |
|---|---|---|
| A — The held call | `Pending`, then resolved allow/deny by a second human | Non-admin, with an admin ready in a second session |
| B — The refused secret | `deny`, `policy=secret_scan`, no exemption | Any — run it once as non-admin, once as admin, to show neither gets through |
| C — The blocked tool | `deny`, `policy=tool_blocklist` for a user, `allow` for the identical call as admin | Non-admin, then the same call as admin |

Run one, or all three in sequence — each ends with the readback in the last section, and each is
independent of the others.

---

# A — The held call

1. **The allow, for contrast.** `crm_lead_search` for a lead to work with — say the audience's own
   demo lead. It executes at once; a `decision=allow` row lands. Name the lead and its id.

2. **The first hold — a note.** `note_add` on that lead (`model: crm.lead`, `res_id`, a short body).
   The call does not return. Explain what is happening while it waits: the governance stage answered
   `Pending`; the call is parked on an `approval_requests` row keyed by a hash of who, which server,
   which tool, which arguments — stable across retries, so a retry cannot slip a second copy through.
   After 60 seconds the server hands the wait back as an MRTR `input_required` and the client retries;
   that is the protocol working, not a timeout.

3. **The second hold — a channel post.** `channel_list` to find a demo Discuss channel, then
   `channel_post` to it announcing the lead. It also does not return — a second `Pending` row, on a
   different tool, so the audience sees the hold is a property of the *pattern*, not a one-off on
   `note_add`.

4. **Switch to the admin.** Open Governance → Approvals. Both calls are listed with the exact
   arguments that will run — for the channel post that means the full message body, not a summary.
   - **Approve the channel post.** It posts, and shows up in the channel with the approver's identity
     on the audit row.
   - **Deny the note.** It comes back refused; Odoo was never touched. Both outcomes, one queue.

5. **The race, if a second admin is on stage.** Have them click Deny on the call the first admin
   already approved. Nothing breaks and nothing is overwritten: the first decision stands, the first
   approver stays stamped, the queue simply stops listing it.

Precondition: check the approvals queue is **empty** before you start — a leftover call from a
rehearsal makes the beat ambiguous. Confirm `services/governance/config.yaml` lists `note_add` and
`channel_post` under `require_approval.patterns`.

---

# B — The refused secret

`secret_scan` reads every tool call's arguments for credential shapes — 32 vendor patterns (cloud
keys, GitHub tokens, Stripe, Anthropic, PEM blocks, database URLs) plus a high-entropy backstop — and
refuses the call outright, before Odoo, before SMTP, before any model spend.

**Never type a live-looking credential into this conversation.** The gateway scanner rescans every
turn; a real key prefix in chat blocks the *session*, not just the call. Use the out-of-band script
`demo/governance/06-secret-breach.sh` for real vendor shapes — the audience sees the refusal without
the secret ever appearing on screen. In-conversation, use fake shapes: the scanner keys on shape, so
they trip it just the same.

1. **A normal write, for contrast.** `note_add` on a demo lead with an ordinary body. As a non-admin
   it is *held* (Scenario A); as an admin it lands. Either way it reached the stage after
   `secret_scan`.
2. **The same write with a credential in it.** `note_add` on the same lead, with a body that pastes a
   fake vendor-shaped credential — for example an AWS-style access key id (`AKIA` followed by sixteen
   upper-case letters and digits) or a GitHub-style token (`ghp_` followed by thirty-six letters and
   digits). The call is **refused**: no hold, no approver, no Odoo round trip.
3. **The channel-post variant.** Run `channel_post` with the same snippet. Refused the same way —
   `secret_scan` runs ahead of `require_approval`, so the credential never even reaches the hold.
4. **Admin included.** Have an admin run step 2. The hold they are exempt from does not matter here;
   `secret_scan` refuses them too.

`demo/governance/06-secret-breach.sh` posts four synthetic `PreToolUse` events straight to the
governance endpoint — an AWS key, a GitHub PAT, an RSA private key, and a clean control — with
`assert_decision` checks, so it fails loudly if the backend ever stops refusing. Run it in a terminal
beside the conversation when the audience wants the real vendor shapes.

---

# C — The blocked tool

`tool_blocklist` refuses any tool whose **name** contains `delete`, `drop` or `destroy` for a
user-scope caller. The point of this beat is that the tool is real: `crm_lead_delete` genuinely
unlinks a lead in Odoo — record, chatter, activities, gone. Governance decides by identity, not by
the tool being harmless.

Run steps 1–3 as a **non-admin**; have an admin ready for step 4.

1. **Make something safe to lose.** As the user, `crm_lead_create` with the name
   `DEMO — delete me` and nothing else. Read back its id. This is the only lead this beat may ever
   point `crm_lead_delete` at.
2. **Try to delete it.** `crm_lead_delete` with that id. **Refused** — the reason names the blocklist
   and the substring that matched. The tool never ran: prove it with `crm_lead_get` on the same id.
3. **Say what just happened.** The refusal was decided before the Odoo server was ever spoken to; the
   user's Odoo permissions were not even consulted. The audit row for this call reads
   `policy=tool_blocklist`, not `policy=odoo`.
4. **The same call as an admin.** The admin runs `crm_lead_delete` on the same id. It **executes**:
   the tool reads the lead first, unlinks it, and reports "irreversible". `crm_lead_get` now says the
   lead is not visible. Same tool, same arguments, same server — the only difference was the caller's
   live role in the database.
5. **The live flip, if there is time.** An admin revokes their own throwaway demo-account's role and
   the identical call is refused; re-grants it and it runs — no restart, no new token. Authority is
   data.

`crm_lead_delete` is pointed only at the lead this beat created. Never at a real one, never on
inference, never "to tidy up".

---

# Read back what happened

Close every scenario the same way: prove the verdict from the audit spine, never state it from
memory. Any question about cost or governance for work done outside this skill also lands here.

## Step 1 — Pull the rows

```bash
systemprompt infra logs trace list --limit 10       # MCP tool calls, with trace ids
systemprompt infra logs request list --limit 10     # AI requests: model, tokens, cost, latency
systemprompt infra db query "SELECT decision, tool_name, agent_scope, policy, reason, created_at FROM governance_decisions ORDER BY created_at DESC LIMIT 10"
```

`trace list` is the tool-call side, `request list` is the `/v1/messages` gateway side, and the direct
`db query` is the governance spine itself — the three line up on `user_id`, `tenant_id`, `session_id`
and `trace_id`. Narrow with `--since 1h` when the window matters; `--status failed` exists on
`trace list`, not on `request list`.

## Step 2 — Reconstruct one call in full

```bash
systemprompt infra logs audit <request-id>          # identity → policy evals → prompt → response → cost
systemprompt infra logs trace show <trace-id>       # PreToolUse → decision → spawn → result
```

This is what makes the governance claim checkable rather than asserted: the audit row names the
identity that made the call, every policy stage that evaluated it, and the decision each returned.

## Step 3 — State the number

**Report the cost in dollars from the data. Never estimate, never round to a "roughly", and never
infer a price from token counts and a rate card you remember.** If the rows do not carry a cost yet,
say the cost has not landed rather than supplying one.

```bash
systemprompt analytics costs summary
systemprompt analytics requests stats
systemprompt analytics costs breakdown --by agent     # spend attributed per agent
```

**The cost angle.** Every denial and every hold costs **$0 of model spend while it waits or is
refused** — a denied call never reaches a provider, and a held call is not executing. Show it: no new
priced request rows for the denied/held calls, against an allowed call's priced row beside them.
Guardrails that fire *before* the spend are themselves a cost feature.

## Two distinct rate limiters (do not conflate them)

There are **two** independent limiters; only the first is the governance stage above:

- **Governance `rate_limit` policy** — per-identity, 300 calls / 60s, configured in
  `services/governance/config.yaml`. Its evidence lives in `governance_decisions` with
  `policy = 'rate_limit'`.
- **HTTP profile limiter** — a separate request limiter shown by `systemprompt admin config
  rate-limits show`. It guards the HTTP surface, is configured in the profile, and is **disabled in
  the local profile**. It does not write `governance_decisions` rows.

## Forcing a specific decision (deterministic demo)

To force an exact stage without a scenario in the loop, POST a synthetic `PreToolUse` event straight
to the governance endpoint — what the repo's `demo/governance/*` scripts do.

For `scope_check` and `tool_blocklist`, use the **user-scope** token (`demo/.token.user`), not the
admin `demo/.token` — both policies exempt admins. `00-preflight.sh` provisions
`demo/.token.user` by minting a plugin token for `demo_user@demo.local` and demoting it to the `user`
role; governance reads the role live, so the token resolves to User scope.

```bash
# scope_check deny: a user-scope caller reaching for an admin MCP tool
curl -s -X POST "http://localhost:8080/api/public/hooks/govern?plugin_id=enterprise-demo" \
  -H "Authorization: Bearer $(cat demo/.token.user)" -H "Content-Type: application/json" \
  -d '{"hook_event_name":"PreToolUse","tool_name":"mcp__systemprompt__list_agents","agent_id":"associate_agent","session_id":"demo-scope","cwd":"/var/www/html/systemprompt-template"}'
# -> {"permissionDecision":"deny", "reason": ...}   (deny — user scope, admin-only tool)

# tool_blocklist deny: a destructive tool name (delete/drop/destroy) blocked for user scope.
# Use a NON-admin-prefixed name (delete_records, not mcp__systemprompt__delete_*): scope_check runs
# first and would short-circuit an admin-prefixed tool, attributing the deny to scope_check. A
# non-prefixed name passes scope_check and is denied by tool_blocklist.
curl -s -X POST "http://localhost:8080/api/public/hooks/govern?plugin_id=enterprise-demo" \
  -H "Authorization: Bearer $(cat demo/.token.user)" -H "Content-Type: application/json" \
  -d '{"hook_event_name":"PreToolUse","tool_name":"delete_records","tool_input":{"table":"users"},"agent_id":"associate_agent","session_id":"demo-blocklist","cwd":"/var/www/html/systemprompt-template"}'
# -> {"permissionDecision":"deny", "reason": "...blocked by list delete"}   (policy=tool_blocklist, user scope)
```

For `secret_scan`, the token choice is the opposite: this stage fires for **any** scope, so use the
admin `demo/.token` to prove even an admin caller is blocked. Put a plaintext credential anywhere in
`tool_input`. The runnable recipe with real test credentials is
`demo/governance/06-secret-breach.sh` (out-of-band `curl`, so the secret never enters this
conversation).

Sending the scope/blocklist requests with the admin `demo/.token` returns `allow` — admins are
exempt from those two policies.

## Rules

- Every claim about a decision is read back from `governance_decisions` — show the row.
- Leave nothing parked when a demo ends: resolve or let expire (15 minutes, `require_approval`'s
  `expiry_seconds: 900`).
- If the scanner does not fire on a fake shape, say so and switch to the script — do not "fix" the
  demo by pasting something more realistic.
- A call that was **denied** or **held** is a successful readback, not a failure — say which stage
  returned the verdict and, for a hold, who it is waiting on.
- `decision=allow, policy=governance_disabled` means the chain ran with a stage switched off; it
  still audited. Report it as it reads rather than as "ungoverned".
