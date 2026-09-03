# The Enterprise Demo — "Claude Force"

**The aim.** One sitting, one admin, five beats, and by the end the audience has watched an AI
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
| 1 | SDR Agent — lead qualification | The morning briefing | `show_activity` (admin) | A series of Odoo queries — `business_overview_data`, `crm_lead_report`, `note_search`, `calendar_event_list`, `task_list`, `activity_list` — into one brief; per-user Odoo record rules; the business dashboards (two admin-only, four from the workspace bundle every role holds) |
| 2 | Service Agent + Data Cloud 360 | Walk my leads | `update_leads` (admin) | Per lead: `crm_lead_get`, `note_list`, `activity_list`, then a status question and the smallest write that answers it: `crm_lead_update`, `note_add`, `activity_create` |
| 3 | Sales Engagement — follow-ups | Governed writes | `lead_factsheet` + `demo_approval_hold` (user) | `crm_lead_create`, `factsheet_render`, `attachment_add`, then `note_add` and `email_send` — as the signed-in salesperson; the outbound ones **held for human approval** (MCP MRTR, SEP-2322). The email is confirmed in-band by its drafter first, and its provenance logged back to the lead by the same call |
| 4 | Einstein Trust Layer | Governed Operations | `demo_secret_refusal` + `demo_blocked_tool` (user), then `demonstrate_governance` (admin) | A credential refused for every caller at $0; a real destructive tool (`crm_lead_delete`) refused for a user and executed for an admin; then the stage-by-stage tour, the live RBAC flip, and `governance_decisions` audit rows including the approver stamp |
| 5 | Agentforce Analytics + Flex credits | Command Center | `governance_readback` (admin) | `infra logs request/trace`, `analytics costs/requests/tools`, `ai_requests` per-request pricing, the three control-plane dashboards |

The thread through all five: **self-hosted, per-user identity (no service account), every call audited
with a trace id, every task priced to the cent — and three outcomes, not two: allow, deny, and *held
for a human*.** Each beat ends with a cost readback via `governance_readback`, so the finale's
total is already earned.

**The demo plugin stages verdicts, not narration.** `systemprompt-demo` ships three skills, one per
verdict the pipeline can return — `demo_approval_hold` (held), `demo_secret_refusal` (refused for
everyone), `demo_blocked_tool` (refused by identity) — plus `lead_factsheet`, the business arc that
ends in a held send. Every step in them is a real tool call the user's own manifest carries. A first
demo plugin whose five skills merely re-narrated the admin ones was deleted; its ids are banned in
`tests/e2e/src/skills_artifacts.rs`.

The demo plugin is granted to `[user]` on purpose: `require_approval` and `tool_blocklist` both exempt
admin callers, so the beats only show for a non-admin. Steps 1, 2 and the readbacks are admin-only
because `show_activity`, `update_leads`, `demonstrate_governance` and `governance_readback` ship in
`systemprompt-admin`, which `services/access-control/roles.yaml` grants to `[admin]`. The enforcement
is that one plugin rule per plugin — the skills carry no rule of their own, and an allow-type skill
rule is forbidden.

## Difficulty ramp

The beats go easy → hard on purpose: 1 is read-only queries into a brief; 2 is a per-record
conversation ending in writes; 3 adds the outbound writes and the approval hold; 4 trips the refusing
stages, flips live policy and reconstructs the audit chain; 5 is the economics close.

---

## Prerequisites

Accounts, passwords, and how to run the demo as a real user instead of the seeded pair: see
**`RUN-DEMO.md`** (the cast list).

1. **Stack up** (`TESTING-INSTRUCTIONS.md` §1): `just db-up local` → `just build` → `just start` →
   `just e2e-live` (seeds `ed@systemprompt.io` / `ed+notadmin@systemprompt.io`, password
   `e2e-live-password-2026`, plus a demo lead).
2. **Seed richer data (optional but better on screen):** create a handful of leads/notes via the `update_leads`
   skill or Odoo UI (`http://localhost:8070`, `admin`/`admin`) so triage has something to rank.
3. **Cowork:** on the demo machine, `systemprompt-internal-bridge login … --gateway http://localhost:8081`,
   `bridge sync`, then **as the admin** run **systemprompt_setup_admin** — the one skill that installs
   dashboards, and admin-only. It installs all eleven: the four workspace dashboards every role's data
   feeds (To-Do Bulletin, Upcoming Deals, Pipeline — Open Deals, Recent Activity — Team Notes) and the
   seven that ride with the admin bundle (business overview, inbound leads, the two brain@ knowledge
   pages and the three control-plane ones). The salesperson holds no setup skill: installing artifacts
   is an admin job.
4. **Verify the manifest** (§3 curl in TESTING-INSTRUCTIONS.md): both users carry `send_email`,
   `lead_factsheet` and the three `demo_*` skills; only `e2e-admin` carries `show_activity`,
   `update_leads`, `demonstrate_governance` and `governance_readback`.

## Governance

All four governance stages are **enabled** on local and production alike
(`services/governance/config.yaml`, settled 2026-08-27) — step 4 works out of the box. Before a demo,
confirm with any in-scope tool call: the audit row should carry a real policy id, not
`policy=governance_disabled`. Never disable a stage by deleting the file — absence means all stages
enabled by default; explicit `enabled: false` is the only reliable off.

The gateway safety scanners (`services/gateway/policies.yaml`) are a separate plane and stay off.

---

## The runbook

Run in order; each skill's SKILL.md is the script and ends by handing off to the next.

**Act 0 — as `e2e-admin`, the business:**
1. `/show_activity` — a series of Odoo queries into one brief: pipeline by stage and owner, the week's
   new leads, what people wrote, meetings, open work, what is overdue. Open the **Leads — Inbound
   Prospects** dashboard, which fetches over the same wire, same identity, same audit row. Flag the
   hottest lead by name and Odoo id. Close with `/governance_readback` for the first cost line.
2. `/update_leads` — walk that admin's own leads one at a time: where it is, last touch, what is
   scheduled, then "what's the status?" — and the smallest write that answers it (`crm_lead_update`
   for a stage move, `note_add` for the reasoning, `activity_create` for the promise). The ledger at
   the end names every id touched, old → new.

**Act I — as `e2e-sales` (a plain user):**
3. `/lead_factsheet` then `/demo_approval_hold` — the writes, as the salesperson. Capture a lead,
   render it a branded sheet, attach it, and email it; then the hold beat proper. The Odoo writes
   land unattended; the outbound-facing ones (`note_add`, `email_send`) **block** — governance holds
   them and no Odoo round trip and no SMTP connection happens at all. `email_send` is confirmed
   in-band by its drafter *before* it parks, so the audience sees both layers: the writer checking
   their own text, then a different person releasing it. Leave both waiting and switch to Act II.
   **Run Act I as `e2e-sales`, never as an admin:** `exempt_scopes: [admin]` exempts an admin
   *requester*, so an admin demo shows no hold at all and looks like the control is broken.

**Act II — as `e2e-admin`:**

3b. **Approve the held calls** — the second half of `demo_approval_hold`, from the admin's chair:
   Governance → Approvals lists both parked calls with the exact arguments that will run. Approve the
   email, deny the note — both outcomes on one queue, the approver stamped on the row. The race with a
   second admin is in the skill; pinned by `a_decision_already_taken_cannot_be_overwritten`.
4. Back as `e2e-sales`: `/demo_secret_refusal` — a credential in a note body, refused before the
   hold at $0, admin included; `/demo_blocked_tool` — a throwaway lead, `crm_lead_delete` refused by
   name, the lead still there; then the same call as `e2e-admin` executes it. Then
   `/demonstrate_governance` (admin) — the stage-by-stage tour, the live RBAC flip, and `infra logs
   audit` reconstructing the chain, including the held calls' `pending` rows and the approver stamped
   on the row that resumed them. Denied and held calls both cost $0: enforcement fires before the
   model spend, and a call waiting on a human is not burning anything.
5. `/governance_readback` — the close: every request and tool call of the sitting, priced from the
   rows, plus the three control-plane dashboards, and the bill: *"everything you just watched cost
   $X.XX"* vs Agentforce's ~$2/conversation list price — labelled as list price, never estimated.

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
  Backporting this runbook there (swapping Odoo for its documentation MCP) is the path to a
  self-serve public version of this demo.
