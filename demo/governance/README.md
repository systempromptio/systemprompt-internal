<div align="center">
  <a href="https://systemprompt.io">
    <img src="https://systemprompt.io/logo.svg" alt="systemprompt.io" width="150" />
  </a>
  <p><strong>Production infrastructure for AI agents</strong></p>
  <p><a href="https://systemprompt.io">systemprompt.io</a> • <a href="https://systemprompt.io/documentation">Documentation</a> • <a href="https://github.com/systempromptio/systemprompt-core">Core</a> • <a href="https://github.com/systempromptio/systemprompt-template">Template</a></p>
</div>

---

# Governance Demos

Tool access control, scope enforcement, secret detection, and audit trails.

## Prerequisites

Run `../00-preflight.sh` first to start services and acquire a token.

## Scripts

| # | Script | What it proves | Cost |
|---|--------|---------------|------|
| 01 | happy-path.sh | Governance ALLOWS admin-scope tool call, MCP tool executes | Free |
| 02 | refused-path.sh | Governance DENIES user-scope agent calling admin tool | Free |
| 03 | audit-trail.sh | Both decisions queryable in governance_decisions table | Free |
| 04 | governance-happy.sh | Detailed rule evaluation — all 3 rules pass for admin agent | Free |
| 05 | governance-denied.sh | Scope check + blocklist deny for user agent | Free |
| 06 | secret-breach.sh | Secret detection blocks leaked credentials in tool inputs | Free |
| 07 | rate-limiting.sh | Rate limit, security, and server configuration | Free |
| 08 | hooks.sh | Hook listing and validation across all plugins | Free |
| 09 | pi-agent.sh | Same pipeline governs Pi, a third-party coding agent: prompt gate + tool gate | Free (`--live` costs tokens) |

## How it works

Demos 01-06 call the governance API directly with `curl`, simulating Claude Code's PreToolUse hook workflow. No AI calls, deterministic, instant.

Demo 09 does the same for Pi, which has no hook named `PreToolUse` — its
`input`, `tool_call`, and `tool_result` events fill the same three roles, and
`examples/pi/extensions/governance.ts` maps them onto the identical endpoint.
The `input` gate is the one Claude Code's tool-level hook cannot reach: a
credential pasted into a prompt is blocked before it is serialized into a
provider request at all.

Calling `/v1/messages` with a personal access token needs one extra step: the
gateway attests the `x-session-id` header against a session row it issued, so a
PAT caller mints one first.

```bash
SESSION=$(curl -fsS -X POST "$BASE_URL/api/public/gateway/sessions" \
  -H "x-api-key: $PAT" | jq -r .session_id)
```

An invented session id is rejected with a 401. JWT callers need nothing extra:
the header is the token's own `session_id` claim.

The governance pipeline:
1. JWT validation (token authentication)
2. Scope resolution (admin vs user agent)
3. Rule engine (scope_check, secret_detection, rate_limit)
4. Audit write (async database INSERT)
5. Response (ALLOW or DENY)


---

## License

MIT - See [LICENSE](https://github.com/systempromptio/systemprompt-template/blob/main/LICENSE) for details.
