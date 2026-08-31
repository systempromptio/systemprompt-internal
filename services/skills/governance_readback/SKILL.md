# Governance Readback

Close the loop on work that just happened: show what it cost and how it was governed. Every
inference call and every MCP tool call lands a row in the audit spine, so the answer is always
queryable — this skill is the one place that says how to ask.

Any skill that needs to state a cost or prove a call was governed defers here rather than
restating the commands.

## Ask me things like

- "What did that cost?"
- "Show me the audit trail for what we just did."
- "Was that call governed?"
- "What have I spent this hour?"

## Step 1 — Pull the rows

```bash
systemprompt infra logs trace list --limit 10       # MCP tool calls, with trace ids
systemprompt infra logs request list --limit 10     # AI requests: model, tokens, cost, latency
```

`trace list` is the tool-call side, `request list` is the `/v1/messages` gateway side. Both are
backed by the same tables keyed on `user_id`, `tenant_id`, `session_id` and `trace_id`, so they
line up. Narrow with `--since 1h` when the window matters; `--status failed` exists on
`trace list`, not on `request list`.

## Step 2 — Reconstruct one call in full

Take a `request_id` or `trace_id` from Step 1 and expand it:

```bash
systemprompt infra logs audit <request-id>          # identity → policy evals → prompt → response → cost
systemprompt infra logs trace show <trace-id>       # PreToolUse → decision → spawn → result
```

This is what makes the governance claim checkable rather than asserted: the audit row names the
identity that made the call, every policy stage that evaluated it, and the decision each returned.

## Step 3 — State the number

**Report the cost in dollars from the data. Never estimate, never round to a "roughly", and never
infer a price from token counts and a rate card you remember.** If the rows do not carry a cost
yet, say the cost has not landed rather than supplying one.

For a wider window than the last few calls, roll up instead of summing by hand:

```bash
systemprompt analytics costs summary
systemprompt analytics requests stats
```

## Rules

- Everything reported comes from this run's rows. Ids from the tool output, costs from the logs.
- A call that was **denied** or **held** is a successful readback, not a failure — say which stage
  returned the verdict (`scope_check`, `secret_scan`, `tool_blocklist`, `rate_limit`,
  `require_approval`) and, for a hold, who it is waiting on.
- `decision=allow, policy=governance_disabled` means the chain ran with a stage switched off; it
  still audited. Report it as it reads rather than as "ungoverned".
- Admins who need the fleet-wide view (all users, all agents, rollups) want `report`; admins
  debugging one failure want `inspect`. This skill is the per-task readback anyone can run.
