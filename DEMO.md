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
| 1 | SDR Agent — lead qualification | The morning briefing | `business_overview` | `business_overview_data` typed table, per-user Odoo record rules, leads dashboard artifact |
| 2 | Service Agent + Data Cloud 360 | Account 360 Brief | `crm` | Multi-tool orchestration on one lead: `crm_lead_get`, `note_list`, `activity_list` |
| 3 | Sales Engagement — follow-ups | Governed writes | `crm` + `lead_factsheet` | `activity_create`, `calendar_event_create`, `note_add`, `email_send` — as the signed-in user; the outbound ones **held for human approval** (MCP MRTR, SEP-2322). The email is confirmed in-band by its drafter first, and its provenance logged back to the lead by the same call |
| 4 | Einstein Trust Layer | Governed Operations | `demonstrate_governance` (admin) | 5-stage governance pipeline, secret scan, live RBAC flip, `governance_decisions` audit rows including the approver stamp |
| 5 | Agentforce Analytics + Flex credits | Command Center | `report` (admin) | `analytics costs/requests/tools`, `ai_requests` per-request pricing, 3 admin dashboards |

The thread through all five: **self-hosted, per-user identity (no service account), every call audited
with a trace id, every task priced to the cent — and three outcomes, not two: allow, deny, and *held
for a human*.** Each beat ends with a cost readback via `governance_readback`, so the finale's
total is already earned.

**There is no demo plugin.** The demo drives the skills a customer actually gets. There used to be
five parallel `demo_*` skills that re-narrated the real ones — `demo_lead_triage` was
`crm_lead_search` plus a ranking, and the two admin ones said in their own bodies that the real
recipes lived in `demonstrate_governance` and `report`. That surface is gone: what the audience
watches is what they are buying, which is the stronger demo anyway.

Steps 4–5 stay admin-only because `demonstrate_governance` and `report` ship in the
`systemprompt-admin` plugin, which `services/access-control/roles.yaml` grants to `[admin]`. The
enforcement is that one plugin rule — the skills carry no rule of their own, and an allow-type skill
rule is forbidden.

## Difficulty ramp

The beats go easy → hard on purpose: 1 is a single tool call; 2 is multi-tool orchestration on one
record; 3 adds governed mutations and the approval hold; 4 flips live policy and reconstructs the
audit chain; 5 synthesizes business reporting, fleet analytics, and the economics close.

---

## Prerequisites

Accounts, passwords, and how to run the demo as a real user instead of the seeded pair: see
**`RUN-DEMO.md`** (the cast list).

1. **Stack up** (`TESTING-INSTRUCTIONS.md` §1): `just db-up local` → `just build` → `just start` →
   `just e2e-live` (seeds `ed@systemprompt.io` / `ed+notadmin@systemprompt.io`, password
   `e2e-live-password-2026`, plus a demo lead).
2. **Seed richer data (optional but better on screen):** create a handful of leads/notes via the `crm`
   skill or Odoo UI (`http://localhost:8070`, `admin`/`admin`) so triage has something to rank.
3. **Cowork:** on the demo machine, `systemprompt-internal-bridge login … --gateway http://localhost:8081`,
   `bridge sync`, run **systemprompt_setup** — one skill for every role and every host; in Cowork it
   installs the four workspace dashboards. An admin then runs **systemprompt_setup_admin**, a separate
   admin-only skill, for the three control-plane dashboards.
4. **Verify the manifest** (§3 curl in TESTING-INSTRUCTIONS.md): both users carry `business_overview`,
   `crm` and `lead_factsheet`; only `e2e-admin` carries `demonstrate_governance` and `report`.

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

**Act I — as `e2e-sales` (a plain user):**
1. `/business_overview` — one call, the pipeline by stage and the week's new leads as an inline typed
   table; open the **Leads — Inbound Prospects** dashboard, which fetches over the same wire, same
   identity, same audit row. Flag the hottest lead by name and Odoo id. Close with
   `/governance_readback` for the first cost line.
2. `/crm` — drill into that lead: `crm_lead_get`, its chatter via `note_list`, its open activities.
   The 360 brief, assembled from typed results rather than scraped screens.
3. `/crm` then `/lead_factsheet` — the writes, as the salesperson. Log a note and schedule a
   follow-up activity; then render the lead a branded factsheet and email it. The Odoo writes land
   unattended; the outbound-facing ones (`note_add`, `email_send`) **block** — governance holds them
   and no Odoo round trip and no SMTP connection happens at all. `email_send` is confirmed in-band by
   its drafter *before* it parks, so
   the audience sees both layers: the writer checking their own text, then a different person
   releasing it. Leave both waiting and switch to Act II.
   Then the beat that never reaches an approver: a second send whose body pastes a config snippet
   containing an `sk-ant-…` key is refused outright by `secret_scan` on the arguments — before SMTP,
   before the hold, at **$0**. Optionally also the record-rules denial beat (a record the salesperson
   cannot touch — Odoo itself refuses). Show the chatter in Odoo under their name.
   **Run Act I as `e2e-sales`, never as an admin:** `exempt_scopes: [admin]` exempts an admin
   *requester*, so an admin demo shows no hold at all and looks like the control is broken.

**Act II — as `e2e-admin`:**

3b. **Approve the held calls.** Open **Governance → Approvals** in the admin console
   (`/admin/governance/approvals`). Both parked calls are listed with the exact arguments that
   will run — for `email_send` that includes the full recipient list and body, so the approver reviews
   what actually goes on the wire, not a summary of it. Approve the email and watch it send and log
   its own provenance back to the lead's chatter. Then deny the held `note_add` and watch it come
   back refused with Odoo never touched — both outcomes, on one queue. The approver is necessarily a different
   person from the requester — an admin's own calls are exempt, so this is a control rather than a
   rubber stamp.

   **The race, if a second admin is on stage.** Have them click Deny on the call the first admin has
   already approved. Nothing breaks and nothing is overwritten: the first decision stands, the
   original approver is still the one stamped in the audit row, and the queue simply stops listing
   it. `ApprovalRepository::resolve` returns `Ok(None)` for a row that is already decided or expired,
   and the console renders that as an ordinary outcome rather than an error — so a late click cannot
   revive an abandoned call and two admins cannot both own one decision. Pinned by
   `a_decision_already_taken_cannot_be_overwritten`.
4. `/demonstrate_governance` — its §"Running it as the demo beat": the allow, three engineered
   denials (scope, secret, blocklist), the
   live RBAC flip, then `infra logs audit` reconstructing the chain — now including the held calls'
   `pending` rows and the approver stamped on the row that resumed them. Denied and held calls both
   cost $0: enforcement fires before the model spend, and a call waiting on a human is not burning
   anything.
5. `/report` — its §"Running it as the demo close": pipeline report, fleet analytics, the three admin
   dashboards, and the bill: *"everything you just watched cost $X.XX"* vs Agentforce's
   ~$2/conversation list price.

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
