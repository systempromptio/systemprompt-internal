# Demo 4 — Governed Operations

The trust-layer use case. Salesforce's answer is the Einstein Trust Layer — a described property.
Here the answer is a synchronous four-stage policy pipeline on **every** tool call, writing one auditable
row per decision to Postgres. This step trips it on purpose, three ways, then proves each denial from
the data.

Admin-only. Two sibling admin skills carry the deep recipes — `demonstrate_governance` (the four
stages, with runnable curl recipes and token guidance) and `manage_platform` (live RBAC flips).
This skill sequences them into the demo beat; follow their instructions for each move rather than
improvising payloads.

## Precondition (check first)

All four governance stages are **enabled** in this installation
(`services/governance/config.yaml`). Confirm before the beat: any in-scope tool call should land a
`decision=allow` row with a real policy id — a row reading `policy=governance_disabled` means the
stages were switched off; re-enable them and restart before demoing. Never change state by deleting
the file: a missing file means all stages ENABLED by default.

## Script

1. **The allow** — run a normal in-scope tool call (e.g. a `crm_lead_search`). It executes; a
   `decision=allow` row lands.
2. **Scope deny** — as a *user-scope* identity, attempt an admin-prefixed tool
   (`mcp__systemprompt__*`). Denied by `scope_check` before execution. Use the recipe in
   `demonstrate_governance` — the user-scope token matters; admins are exempt from this stage.
3. **Secret deny** — a plaintext credential in tool input is denied by `secret_scan` for **any**
   scope, admin included. Run `demo/governance/06-secret-breach.sh` out-of-band — never paste a live
   credential prefix into the conversation (the gateway scanner would re-scan it every turn).
4. **Blocklist deny** — a destructively-named tool (`delete_records`) for user scope, denied by
   `tool_blocklist`.
5. **The live flip** — the `manage_platform` beat: revoke a role, the same call is denied; re-grant
   it, the call succeeds. Same bearer token, no restart, no redeploy. Authority is data.
6. **Reconstruct** — prove all of it from the spine:
   ```bash
   systemprompt infra db query "SELECT decision, tool_name, agent_scope, policy, reason FROM governance_decisions ORDER BY created_at DESC LIMIT 10"
   systemprompt infra logs trace list --limit 10
   systemprompt infra logs audit <request-id>       # identity → policy evals → prompt → cost, one chain
   ```

## Cost readback

Every denial above cost **$0 of model spend** — denied calls never reach a provider. Show it:
`systemprompt infra logs request list --limit 5` (no new requests for the denied calls) versus the
allow's priced row. Guardrails that fire *before* the spend are themselves a cost feature.

## Rules

- Every claim about a decision must be read back from `governance_decisions` — show the row.
- Restore any role you revoked and (if the demo is over) re-disable the four stages per DEMO.md.
- Hand off: end by offering the finale, `demo_command_center`.
