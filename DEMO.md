# The Enterprise Demo — "Claude Force"

**The aim.** One sitting, one admin, five skills, and by the end the audience has watched an AI
workforce triage leads, brief accounts, execute governed writes into the system of record, trip a real
trust layer, and then read back the **exact cost of everything it just did** — set against Salesforce
Agentforce's ~$2-per-conversation list price. The claim is not "we are like Agentforce but cheaper";
it is that every capability Agentforce describes is here as **queryable data**: per-user identity,
per-decision audit rows, per-task cost.

The demo runs through Claude Cowork exactly as described in `TESTING-INSTRUCTIONS.md`: marketplace →
plugins → skills (`/` picker) → MCP tools returning typed structured results → inline tables and
dashboard artifacts → the `infra logs` / `analytics` readback.

---

## The five use cases (Agentforce → Claude Force)

| # | Agentforce sells | We run | Skill | Underlying machinery |
|---|---|---|---|---|
| 1 | SDR Agent — lead qualification | Lead Inbox Triage | `demo_lead_triage` | `crm_lead_search` typed table, per-user Odoo record rules, leads dashboard artifact |
| 2 | Service Agent + Data Cloud 360 | Account 360 Brief | `demo_account_360` | Multi-tool orchestration across two MCP servers (odoo + knowledge-bank) |
| 3 | Sales Engagement — follow-ups | Follow-Up Orchestrator | `demo_followup_orchestrator` | Governed writes: `activity_create`, `calendar_event_create`, `note_add`, `channel_post` — as the signed-in user |
| 4 | Einstein Trust Layer | Governed Operations | `demo_governed_operations` (admin) | 4-stage governance pipeline, secret scan, live RBAC flip, `governance_decisions` audit rows |
| 5 | Agentforce Analytics + Flex credits | Command Center | `demo_command_center` (admin) | `analytics costs/requests/tools`, `ai_requests` per-request pricing, 3 admin dashboards |

The thread through all five: **self-hosted, per-user identity (no service account), every call audited
with a trace id, every task priced to the cent.** Each skill ends with a cost readback so the finale's
total is already earned.

## Difficulty ramp

The skills go easy → hard on purpose: 1 is a single tool call; 2 is cross-server orchestration; 3 adds
governed mutations and a permission-denial beat; 4 flips live policy and reconstructs the audit chain;
5 synthesizes business reporting, fleet analytics, and the economics close.

---

## Prerequisites

1. **Stack up** (`TESTING-INSTRUCTIONS.md` §1): `just db-up local` → `just build` → `just start` →
   `just e2e-live` (seeds `e2e-admin@systemprompt.local` / `e2e-sales@systemprompt.local`, password
   `e2e-live-password-2026`, plus a demo lead).
2. **Seed richer data (optional but better on screen):** create a handful of leads/notes via the `crm`
   skill or Odoo UI (`http://localhost:8070`, `admin`/`admin`) so triage has something to rank.
3. **Cowork:** on the demo machine, `systemprompt-internal-bridge login … --gateway http://localhost:8081`,
   `bridge sync`, run **systemprompt-setup-cowork** (installs the six user dashboards); as admin also run
   **admin_workspace_setup_cowork** (the three admin dashboards).
4. **Verify the manifest** carries the five demo skills (§3 curl in TESTING-INSTRUCTIONS.md):
   steps 1–3 for both users, 4–5 only for `e2e-admin`.

## Governance switch (needed for step 4 only)

All four governance stages are intentionally **disabled** here (`services/governance/config.yaml`).
For the demo:

1. Flip the four `enabled: false` → `true` in `services/governance/config.yaml` (never delete the file —
   absence means all stages enabled by default, which is the confusing way to find out).
2. Restart the server (`just start` after a stop; check `just server-status` first — never trample
   another agent's server).
3. After the demo, flip them back and restart. The audit spine records rows either way — while disabled,
   rows read `decision=allow, policy=governance_disabled`, which is itself worth showing.

The gateway safety scanners (`services/gateway/policies.yaml`) are a separate plane and stay off.

---

## The runbook

Run in order; each skill's SKILL.md is the script and ends by handing off to the next.

**Act I — as `e2e-sales` (a plain user):**
1. `/demo_lead_triage` — one call, ranked leads, inline typed table, leads dashboard, first cost line.
2. `/demo_account_360` — four reads across two servers on the hottest lead; the 360 brief.
3. `/demo_followup_orchestrator` — four writes into Odoo as the salesperson; optionally the denial beat
   (a record the salesperson cannot touch — Odoo itself refuses). Show the chatter in Odoo under their
   name.

**Act II — as `e2e-admin`:**

4. `/demo_governed_operations` — the allow, three engineered denials (scope, secret, blocklist), the
   live RBAC flip, then `infra logs audit` reconstructing the chain. Denied calls cost $0 — enforcement
   fires before the model spend.
5. `/demo_command_center` — pipeline report, fleet analytics, the three admin dashboards, and the bill:
   *"everything you just watched cost $X.XX"* vs Agentforce's ~$2/conversation list price.

Every number in the demo comes from the running system. If a step can't prove its claim from a command
output, the skills are written to say so rather than improvise.

## Troubleshooting

`TESTING-INSTRUCTIONS.md` §8 in this order: `infra logs view --level error --since 10m` →
`infra services status` → `plugins mcp logs odoo` → `infra logs trace list` → `infra logs audit <id>`.
Empty dashboards mean Odoo answered empty (check the artifact's embedded tool contract via MCP
Inspector), not a UI bug. A 401 on the MCP proxy is a token/resource mismatch — bridge logout/login.

## Beyond this repo (productionizing the demo)

- `../systemprompt-template` carries the governance-first variant: 44 self-asserting demo scripts, the
  `use_dangerous_secret` / `manage_permissions` showcase skills, air-gap and scaled-deployment proof
  scenarios with committed results, and recording/SVG assets.
- `../systemprompt-demo` is demo.systemprompt.io — the public marketplace and Bridge onboarding flow.
  Backporting these five skills there (swapping Odoo for its documentation MCP) is the path to a
  self-serve public version of this demo.
