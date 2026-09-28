# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# Systemprompt Internal

**Use the CLI to discover commands.** `systemprompt --help` is your starting point.

---

## Branching & Release Flow

**All work lands on `next`. Never push to `main`.** `next` is the default
branch; `main` is protected by a ruleset that requires a pull request and
grants no bypass to anyone, so a direct push is refused for every agent,
session and admin. Full contract: `docs/BRANCHING.md`; procedure:
`docs/RELEASING.md`.

```
next   ← default branch. Every push runs .github/workflows/gates.yml.
  ↓ `just release X.Y.Z` — frozen promotion PR, merged only on proof
main   ← release-only. A push here IS the release (release.yml).
```

- **Gates run on every push to `next`** (and on ordinary PRs): static
  (fmt, sqlx cache, source gates, release-script self-tests), lint (clippy
  ×3, rustdoc, MSRV), test (unit, integration, contract, e2e), bridge, and
  supply chain, reported together; `Gates passed` is the one aggregate the
  `main` ruleset requires. `just verify` is the same set locally.
- **`just release X.Y.Z`** refuses unless the tree is clean, HEAD is
  `origin/next`, the patch is dormant, every pin agrees (lockstep:
  `scripts/sync-release-version.sh X.Y.Z --check`,
  `scripts/check-release-version.sh`, `scripts/check-core-ref.sh`), and the
  latest next-push Gates run on that exact SHA is green. It pushes
  `promote/X.Y.Z/<main>/<next>` and opens the PR, whose Gates run only
  re-verifies that proof and the merge tree. Repeat `just release X.Y.Z` once
  that run finishes: it re-checks everything and merges with the expected head.
- **`release.yml`** on the merge verifies the promotion
  (`scripts/check-release-merge.sh`), builds `bridge-vX.Y.Z` and `vX.Y.Z`,
  and publishes the image as `:sha-<short>` only; `smoke` and `upgrade-boot`
  prove that digest before `promote-tags` points `:X.Y.Z`/`:X.Y`/`:X`/`:latest`
  at it. Nothing is tagged by hand.
- **Versioning is lockstep:** this repo's version IS the core version it
  builds against (workspace, bridge, image, every core pin, `bridge/CORE_REF`
  = `vX.Y.Z` while the patch is dormant). `scripts/sync-release-version.sh`
  writes all of it; `scripts/sync-core-version.sh` is its core-pin half.
- **Working against unreleased core:** activate the patch in both
  `Cargo.toml` and `tests/Cargo.toml` (rename the dormant
  `[workspace.metadata.unreleased-core-patch]` table to `[patch.crates-io]`,
  with the `# ACTIVE: core X.Y.Z is unreleased` marker the pre-commit hook
  requires), bump the pins to the
  sibling's version, `just core-pin` so `bridge/CORE_REF` names the core
  commit (push core first), and deploy only through `just deploy`, whose
  `core-guard` refuses a dirty or unpinned sibling. `main` never carries an
  active patch.

## Quick Start

```bash
# First-time setup: writes .systemprompt/profiles/local/, starts Docker Postgres,
# runs publish_pipeline. With no key arg, the CLI prompts for which provider to
# use; the chosen provider becomes ai.default_provider (others disabled) and the
# gateway default. Passing keys is non-interactive — the first becomes default.
just setup-local                                                          # interactive provider pick
just setup-local <anthropic_key> [openai_key] [gemini_key] [http_port=8080] [pg_port=5432]

# Build (auto-uses live DB if reachable, else SQLX_OFFLINE=true)
just build            # debug
just build --release  # release

# Lint (workspace, -D warnings, same offline fallback as build)
just clippy

# Regenerate .sqlx/ offline query cache (needs live DB)
just prepare

# Start services — runs the DEBUG binary. The local iterate loop is
# `just build` + `just start`; release builds are for deploys/packaging only,
# never for a local restart.
just start

# Discover CLI commands
systemprompt --help

# List skills
systemprompt core skills list
```

---

## Shared Build State (read this when you are about to compile)

Several agents work this clone at once. Builds, clippy, and tests are expensive
and take a shared cargo lock, so a build started mid-iteration stalls everyone.

**Do all the work first, validate once at the end.** Never run `just build` or
`just clippy` between edits to see how you're doing; finish the change set, then
run the gate a single time.

**Once a task has you compiling, check the shared state before spending anything:**

```bash
just build-status     # in-flight run + last result per recipe (a record, never a reason to skip)
just server-status    # running server, its binary, and whether that binary is stale
```

`just build`, `just clippy`, `just test-*`, and `just lint-gates` are
single-flight (`scripts/build-coordinator.sh`). One rule: **one build in
flight at a time, always of the latest source.** Every call compiles the tree
as it is (cargo is incremental, so an unchanged tree costs seconds); there is
no "already built, skip" cache — one returned "already green" while a bare
`cargo build` from another tree had overwritten `target/debug/systemprompt`
with stale code.

| situation | what happens |
|-----------|--------------|
| a run of this exact source is in flight | you are told so, attach to its log, exit with its status |
| a run of different source is in flight | you are told so, wait for it, then run over the latest tree |
| nothing in flight | you lead |

**Free-disk guard.** Before leading any run except the read-only
`lint-gates`, the coordinator refuses to start when the volume holding the
cargo target dir (`$CARGO_TARGET_DIR`, else `./target`) has less than
`BUILD_MIN_FREE_GB` free — **default 25 GB**. It names the free space and
stops; it never deletes anything. On 2026-09-28 a build from another session
filled this host's disk: cargo does not stop at "disk full", it dies mid-link
with errors that read as compile failures, and Postgres, the server and every
other agent's build fail with it. Free space first (a stale tree's `target/`,
`coverage-report/`, old worktrees); lower the floor only deliberately
(`BUILD_MIN_FREE_GB=15 just build`), and `BUILD_MIN_FREE_GB=0` disables it.
CI bypasses the coordinator (`BUILD_NO_COORD=1`) and so the guard.

Results land in `.build/` (gitignored): `runs.jsonl`, `latest/<recipe>.json`,
`logs/`, `binaries.jsonl`. Read them instead of re-running.

`just start` reports the running server first, then starts. It does not restart
a server another agent is already running (say so and stop), and it warns when
the binary predates the current source, but it only refuses outright when there
is no binary at all. `just stop` shuts this clone's services down cleanly.

Always go through the justfile. A bare `cargo build` bypasses coordination and
the disk guard, and re-creates the contention. Escape hatches when you truly
need them: `START_FORCE=1`, `BUILD_NO_COORD=1` (`BUILD_FORCE=1` is accepted
and does nothing — every run already compiles).

---

## Preflight (the gates; CI re-runs them on every push to `next`)

```bash
just verify             # what gates.yml runs: static → lint → tests
just preflight          # verify + the coverage floor/ratchet
just preflight-static   # seconds: fmt ×3, sqlx cache, core crate versions, the source gates
just preflight-lint     # clippy (all three workspaces), doc-check, msrv-check
just preflight-full     # weekly: preflight + deny + audit + machete + hack (all workspaces)
just test               # unit → integration → contract → e2e; every tier runs, every failure is listed
just init-hooks         # once per clone: tracked .githooks/ (pre-commit only; no pre-push hook)
```

`.github/workflows/gates.yml` runs the full matrix on each `next` push and on
ordinary PRs; frozen release PRs verify that exact push run instead of
repeating it. The tiers are independent, so a red static tier never hides a
clippy or test failure. Every check in `preflight-static`/`preflight-lint`
runs even after one fails.

**Source gates.** `just lint-gates` runs every script in the justfile's
`gates=()` array concurrently and reports every failure (trust the array, not
a count here). Several carry **known debt as an explicit list** that fails on
a stale entry, so it can only shrink: `check-dropped-schema.sh`
(`KNOWN_UNDROPPED`), `check-template-fields.sh`
(`scripts/template-fields-exemptions.txt`), and `check-schema-baseline.sh`
(skips until the first ladder rung exists). `check-discarded-results.sh` and
`check-fail-open.sh` run core's `scripts/rust-contracts` scanners from the
sibling checkout: without one they skip loudly locally and fail under CI.
`check-fork-drift.sh` compares against `SIBLING_REPO` (default
`../systemprompt-template`) — point it at a checkout of the template's
`next`, or it reports the sibling's staleness, not yours.

**End-to-end suite (`tests/e2e`).** The full production API router in-process
— per-role `/v1/bridge/manifest` diffs, Odoo sign-in with group→role mapping
(wiremock Odoo), the real `systemprompt-mcp-odoo` binary over the MCP wire,
and skill/artifact bundle delivery. It is `just test`'s last tier
(`test-e2e`) and runs in CI; `just e2e` runs it alone, `just e2e-fast` skips
the MCP subprocess, and `just e2e-live` walks the two-role journey (seeded
`e2e-admin@` / `e2e-sales@` Odoo users, PKCE sign-in, manifest diff, chatter
via the MCP proxy) against the RUNNING local stack — it reuses your server
and Odoo and never restarts anything.

**Coverage floor + ratchet.** `just coverage` runs an instrumented llvm-cov
pass over all three workspaces (root, `tests/`, `bridge/`) into
`coverage-report/` (gitignored); `just coverage-check` enforces the tracked
`coverage/baseline.json` — a global floor, a 0.5pt total ratchet, and per-crate
ratchets. If you raised coverage, re-record with `just coverage-baseline`,
then `just coverage-badge`, and commit both; lowering it is a review-visible
act. `coverage-check` refuses to record a baseline under half the previous
total (a run that lost its instrumented binaries reads 0.00%), and the
`coverage-badge.sh` gate fails if the README badge and the baseline disagree.
`.github/workflows/coverage.yml` measures `main`, nightly `next` and on
demand; it is not a release check. Never use cargo-llvm-cov here (it
re-injects the mold linker flags and silently produces zero profraws — see
`scripts/coverage.sh`). No baseline is recorded yet.

**The schema-upgrade ladder.** `tests/fixtures/schema/release-baseline-<X.Y.Z>.sql`
holds one recorded schema per release from the floor (0.61.0,
`scripts/check-schema-baseline.sh`); `just schema-baseline` records the rung
for the workspace version from a fresh local install, and
`just schema-baseline X.Y.Z` records a past release from its published Linux
tarball. The gate requires a rung for every release tag from the floor and one
named for the workspace version, so `just core-bump` turns it red until the
new rung is recorded. Never edit a rung by hand. `release.yml`'s
`upgrade-boot` boots the image over every rung, seeded by
`tests/integration/schema-upgrade/src/seed_hot_tables.sql`, and
`tests/integration/schema-upgrade` restores each rung, seeds it the same way,
runs the current installer over it (2000 rows per hot table must survive) and
diffs the result against a fresh install.

Tests live in the `tests/` workspace (`unit/`, `integration/`, `contract/`,
`e2e/`) or in-crate `tests/` dirs — inline `#[cfg(test)]` modules are banned.
DB-backed suites need Docker Postgres up (`just db-up`), and fail rather than
skip without a database URL.

---

## CLI Structure

```
systemprompt <domain> <subcommand> [args]
```

| Domain | Purpose |
|--------|---------|
| `core` | Skills, content, files, contexts, plugins, hooks, artifacts |
| `infra` | Services, database, jobs, logs |
| `admin` | Users, agents, config, setup, session |
| `cloud` | Auth, deploy, sync, secrets, tenant, domain |
| `analytics` | Overview, conversations, agents, tools, requests, sessions, content, traffic, costs |
| `web` | Content-types, templates, assets, sitemap, validate |
| `plugins` | Extensions, MCP servers, capabilities |
| `build` | Build core workspace and MCP extensions |

**Use `systemprompt <domain> --help` to explore any domain.**

---

## CLI Discovery Workflow

When you need to perform a task, use the CLI help to find the right command:

```bash
# Top-level help
systemprompt --help

# Domain help
systemprompt core --help
systemprompt infra --help

# Subcommand help
systemprompt core skills --help
systemprompt core skills show --help
```

---

## Architecture (big picture)

- `src/main.rs` is a thin entry point that delegates to the published `systemprompt` core crates (sibling checkout at `../systemprompt-core`, patched in via `[patch.crates-io]` for cross-repo work). All customization is **compile-time** via the [`inventory`](https://docs.rs/inventory) crate — there is no dynamic plugin loader.
- Rust code lives in `extensions/`: `extensions/mcp/*` for MCP server extensions, `extensions/web` for page data and template rendering. Each MCP extension has its own crate with `Cargo.toml` + `.sqlx/` offline cache.
- Configuration is YAML under `services/`, loaded through `services/config/config.yaml`'s explicit `includes:` list. Unknown keys error loudly (`#[serde(deny_unknown_fields)]`).
- Governance is a five-stage synchronous pipeline on every tool call: **scope check → secret scan (35+ patterns) → blocklist → rate limit → require approval**. Every decision is audited to Postgres with a trace_id linking identity → agent → tool → result → cost. **All five stages are enabled in this installation, in warn mode** (`services/governance/config.yaml`, top-level `mode: warn`): every stage runs and audits, a confirmed match is recorded as `decision=warn` and nothing refuses — except `require_approval`, which names `mode: enforce` because a warn-mode hold is no approval. The explicit 35-signature `secret_scan.patterns` list is the live catalogue (absence means disabled). The gateway safety scanners (`services/gateway/policies.yaml`) are on too — `[heuristic, secrets, pii_extended]`, `safety.mode: warn`, `quota_mode: warn` — and `quota_fault_mode: closed` in `services/ai/gateway.yaml`. Read all three back with `systemprompt infra logs governance report --since 7d`; put a stage back to `mode: enforce` once it stops misfiring. The refused-path demo shows warnings, not refusals, until then. If a stage is ever disabled, the chain still runs and still audits: calls are recorded as `decision=allow, policy=governance_disabled`. Authentication is separate and is *not* disabled — an invalid or expired token is still denied, with `policy=authentication`. Do not disable governance by deleting the config file: a missing file falls back to core's vendor-neutral warn-only chain, whose secret scanner has no patterns.
- `require_approval` is the fifth stage and the only one that returns a third verdict, `Decision::Pending` — the call is neither allowed nor denied but **held for a named human**. It is deliberately *not* in `GovernanceConfig::defaults()`: every other stage fails toward more enforcement on a bad config read, which is right for a stage that refuses and wrong for one that blocks waiting on a person who may not be watching. It holds nothing until `patterns` names something. A held MCP tool call parks on an `approval_requests` row keyed by a **derived** call id (`sha256(user | server | tool | args digest)`, stable across retries) and blocks; an admin resolves it at `/admin/governance/approvals`; the approver is stamped into the audit row via the `ApproverStamp` field. If the wait outlives one round the server answers with MCP `resultType: "input_required"` (**MRTR**, SEP-2322, protocol `2026-07-28`) and the client retries — which is why all three MCP servers advertise `2026-07-28` and not `2025-06-18`. On the Claude Code `PreToolUse` webhook the same verdict renders as `permissionDecision: "ask"` instead, so the user is prompted in-terminal.
- Per-clone Docker Postgres: `just db-up / db-down / db-logs [tenant=local]`. Project name is derived from a hash of the repo path, so multiple clones on one host get isolated containers and volumes. There is no destructive reset recipe — recover migration checksum drift in place with `just repair-migrations`.
- Deploy flow: `just build-all` (release binary + MCP servers + web assets) then `just deploy`. The `publish_pipeline` job also runs automatically at server startup.

---

## Debugging & Troubleshooting

```bash
# Quick error check
systemprompt infra logs view --level error --since 1h

# Debug AI request failures
systemprompt infra logs request list --limit 10
systemprompt infra logs audit <request-id>

# Debug MCP tool failures
systemprompt plugins mcp logs <server-name>

# Debug agent issues
systemprompt infra logs trace list --agent <agent-name> --status failed
```

**Key debugging workflow:**
1. `infra logs view --level error` — Find the error
2. `infra logs request list` — Find failed AI requests
3. `infra logs audit <id>` — Get full conversation context
4. `plugins mcp logs <server>` or `logs/mcp-*.log` — Get MCP tool errors

---

## Viewing Governance

Every inference call (`/v1/messages`) and every MCP tool call lands a row in the governance spine. Same CLI surface for both — no separate "gateway logs" vs "tool logs":

```bash
# Every AI request — user, model, token counts, cost, latency, status
systemprompt infra logs request list --limit 20
systemprompt infra logs request list --since 1h --provider anthropic   # request list filters: --since / --model / --provider (no --status)
systemprompt infra logs trace list --status failed          # only failed runs — --status lives on trace list, not request list

# Full audit for one request — identity, policy evals, prompt, response, cost
systemprompt infra logs audit <request-id>

# Tool-call traces (PreToolUse → decision → spawn → result)
systemprompt infra logs trace list --limit 20
systemprompt infra logs trace list --agent <name> --status failed
systemprompt infra logs trace show <trace-id>

# Cost + usage rollups (hits the same audit table)
systemprompt analytics costs summary
systemprompt analytics requests stats
systemprompt analytics agents
systemprompt analytics tools
```

`logs request list` shows one row per `/v1/messages` hit — the gateway path Pi / any Anthropic-SDK client uses. `logs trace list` shows MCP tool calls. Both are backed by the same 18-column `ai_requests` / trace tables with `user_id`, `tenant_id`, `session_id`, `trace_id` — so `audit <id>` reconstructs the chain from identity to cost.

**`infra logs` vs `analytics` — operational vs dashboard.** The `infra logs request {list,stats}` commands are quick operational views (recent rows, by-provider / by-model aggregate). Their `analytics requests {list,stats}` counterparts are dashboard metrics over a time range with model filtering, cache-hit rate, and CSV export. Same `ai_requests` table underneath — reach for `infra logs` when triaging a live issue, `analytics` when reporting. The `--help` on each cross-references the other.

For live tailing while reproducing an issue: `infra logs view --follow --since 30s`.

---

## Services Configuration

All runtime configuration lives as YAML under `services/`. The root `services/config/config.yaml` is a thin aggregator with an explicit `includes:` list — every **flat** resource file must be listed there. Skills and plugins are the exception: they are auto-discovered from their nested directories and must *not* be added to `includes:`.

```
services/
  config/config.yaml        Root aggregator (includes the flat resource files)
  agents/<id>.yaml          Flat agent definitions (none ship here — see below)
  mcp/<name>.yaml           Flat MCP server definitions
  skills/<id>/config.yaml   Skill definitions (nested dir, auto-discovered)
  plugins/<id>/config.yaml  Plugin binding descriptors (nested dir, auto-discovered)
  governance/config.yaml    Policy chain (all five stages on, warn mode — see Architecture)
  gateway/policies.yaml     Gateway quotas + safety scanners (both on, warn mode)
  ai/config.yaml            AI provider config
  scheduler/config.yaml     Job scheduler
  web/config.yaml           Web frontend config (full WebConfig)
  content/config.yaml       Content source config
```

Unknown YAML keys cause loud errors at load time (`#[serde(deny_unknown_fields)]`). Nested `includes:` resolve recursively. Plugin YAMLs are binding descriptors that reference top-level agents, skills, mcp servers, and content sources by id — never inline copies.

**This instance ships no A2A agents.** `services/config/config.yaml` says so explicitly: nothing under `services/agents/`, nothing spawned on the agent port range, and no `agents/<id>.md` in any plugin bundle. Skills, MCP servers and artifacts carry the capability instead — so `admin agents list` returning nothing is the correct answer here, not a fault.

---

## Critical Rules

1. **Core is a crate dependency** — consumed from crates.io; the sibling `../systemprompt-core` checkout IS editable for cross-repo work via the `[patch.crates-io]` toggle (publish + bump + re-comment before landing).
   **Adopting a new core version:** bump the pins in **both** `Cargo.toml` and `tests/Cargo.toml` — a stale pin silently drops the patch and resolves the old crate from crates.io, so the build passes having proved nothing. Verify with `scripts/sync-release-version.sh <version> --check` (lockstep: it runs `scripts/sync-core-version.sh` for the core pins) and confirm the build log names the sibling path. Then run migrations with the **new** binary, `just prepare` to refresh the offline cache, and read core's changelog for tightened identifier validators and new `NOT NULL` columns — both are runtime failures that `cargo build` cannot catch (a validated `ContextId` panics in `new()`; a `NOT NULL` column breaks seed migrations that never named it). Full procedure: `docs/RELEASING.md` Step A0.
2. **Rust code -> `extensions/`** — All `.rs` files live here.
3. **Config only -> `services/`** — YAML/Markdown only. No Rust code.
4. **CSS files -> `storage/files/css/`** — NEVER put CSS in `extensions/*/assets/css/`.
5. **Brand name is `systemprompt.io`** — Use "Systemprompt Internal" for the product, "systemprompt.io" for the brand and URLs.
6. **It's a library, not a framework** — Embedded code you own and extend. NEVER call it a "framework".
7. **Demo scripts must work on macOS and Linux** — BSD vs GNU differ on `grep -oP`, `head -n -1`, `sha256sum`, `sed -i`, and binary downloads (pick `hey_darwin_amd64` vs `hey_linux_amd64`). `demo/_common.sh` provides `install_hey()` for the last case; prefer `grep -oE` + `sed -n 's/.../\1/p'` over `grep -oP … \K …`.
8. **Integration work uses typed models** — wire data crosses boundaries as
   `#[derive(Serialize, Deserialize)]` structs (with custom deserializers for
   provider quirks — see `extensions/mcp/odoo/src/server/crm_shape.rs`'s
   `LeadRow` + `odoo::*` adapters), never `json!` literals or `.get()` chains
   over `serde_json::Value`. `Value` survives only at declared protocol
   boundaries carrying a `// JSON: protocol boundary` comment.
9. **No Co-Authored-By in commits** — `coauthorAttribution: false` is set in `.claude/settings.json`. Never add `Co-Authored-By:` trailers to commit messages.

---

## Repository Naming Convention

Every function under `extensions/web/admin/src/repositories/` is named for what
it returns, so a call site reads the same as its signature:

| Returns | Prefix | Example |
|---------|--------|---------|
| `Vec<T>` — zero or more rows | `list_` | `list_top_users` |
| `Option<T>` — a row that may be absent | `find_` | `find_session_header` |
| `T` — exactly one value, or an error | `get_` | `get_request_stats` |
| a page plus its total, `(Vec<T>, i64)` | `list_` | `list_requests_paged` |

Mutations keep the verb that describes them: `insert_`, `update_`, `delete_`,
`set_`, `count_`.

`scripts/check-repository-naming.sh` enforces this: it rejects `fetch_`
outright, and checks every other prefix against the function's actual return
type, so the table above cannot quietly stop being true.

`fetch_` is banned because it is not a synonym for the three above —
it was doing all three jobs at once, which is how the convention drifted: a
reader could not tell from `fetch_summary` whether an absent row was `None` or
an error, and had to open the file to find out.

---

## CSS Files (IMPORTANT)

**All CSS files go in `storage/files/css/`** and must be registered in `extensions/web/src/extension.rs`.

```
storage/files/css/          <- CSS SOURCE (put files here)
extensions/web/src/extension.rs  <- REGISTER here in required_assets()
web/dist/css/               <- OUTPUT (generated, never edit)
```

**To add CSS:**
1. Create file in `storage/files/css/`
2. Register in `extension.rs` `required_assets()`
3. `just publish` to compile templates, bundle CSS/JS, and copy all assets to `web/dist/`

---

## Publishing Assets

After changing templates, CSS, JS, or static files, run:

```bash
just publish
```

This runs (in order): `bundle_admin_css` -> `copy_extension_assets` -> `content_prerender`. Order matters — bundles must be built before `copy_extension_assets` copies them to `web/dist/`. Admin pages are SSR'd at runtime from `.hbs` templates in `storage/files/admin/templates/`, not precompiled.

**Exception: the public-site partials are compiled into the binary.** `services/web/templates/partials/{head-assets,header,footer,scripts}.html` are `include_str!`-embedded by `extensions/web/site/src/partials.rs`. Editing them requires a rebuild (`just build`) and a server restart before `just publish` — running publish alone keeps serving the markup baked into the old binary.

---

## Plugins

Each plugin is a directory holding one `config.yaml` — `services/plugins/<id>/config.yaml` — auto-discovered, so it is **not** listed in the root aggregator's `includes:`. The file's root key is `plugin:` (singular; one plugin per file). It aggregates agents, skills, mcp servers, and content sources by reference:

```yaml
plugin:
  id: systemprompt-business
  name: "Systemprompt Business — Run the Pipeline on Odoo"
  version: "2.0.0"
  enabled: true
  skills:
    source: explicit
    include:
      - manage_leads
      - close_deal
  agents:
    source: explicit
    include: []
  mcp_servers:
    source: explicit
    include: [odoo]
  artifacts:
    source: explicit
    include: []
```

**A plugin is the role boundary.** Every plugin is declared exactly once as `plugin/<id>` in
`services/access-control/rules.yaml`, the one declarative source of entitlement (each entity with a
required `why`; marketplace configs carry no `access:` block) — `[user]` (shared by every role;
admins hold `user` too) or `[admin]`. Until the rules.yaml loader lands the server still reads
`roles.yaml`, and `scripts/validate-services.sh` fails unless the two agree entry for entry — and every skill and artifact inside it inherits that rule. The cascade is
skill/artifact → plugin → marketplace and the nearest level that declares any rule decides, so a
`[admin]` plugin closes its ruleless skills to users even though the marketplace admits them. Never
write a per-skill `allow` rule; never mix scopes in one plugin. Three plugins ship here, 6 skills
in total: `systemprompt-business` (`[user]`, everyone including admins: `activity_report` —
read-only, a business-wide or personal brief, never writes; `manage_leads` — create a lead or walk
your own open leads to a status, stage moves, revenue, notes, follow-ups; `pending_task` — a guided,
one-item-at-a-time sweep of everything outstanding (overdue/due activities, open tasks, stale leads)
that writes back the moment you answer for each one — backing two dashboards — `my-day` (the
briefing, what is waiting on you and the team's chatter) and `sales-pipeline` (every lead and open
deal, in three views). Both **write as well as read**: the stage menu, Won/Lost, the activity tick,
the note and follow-up buttons are all real Odoo writes through the artifact's own `mcp_tools`
allowlist, executed as the signed-in user, so Odoo's record rules are the boundary. Neither may
carry `crm_lead_delete`. This plugin also owns the session-global governance hooks),
`systemprompt-demo` (`[user]`, because the hold and the blocklist exempt admins: one unified skill
`demonstrate_governance` — pick a governance scenario (a held call, a refused secret, a blocked
tool), run it with real tool calls, then read back the audited decision trail), and
`systemprompt-admin` (`[admin]`: `systemprompt_setup_admin` — the control plane's one remaining
skill — plus two dashboards: `admin-activity-requests` and `admin-usage-costs`; the user directory
and the two brain@ knowledge dashboards were retired in favour of `/admin/access/users` and
`/admin/governance/approvals`). Every skill is driven by MCP
tools the holding role's manifest actually carries; the admin CLI passthrough appears only in the
admin plugin. Installing dashboards is admin-only, and `systemprompt_setup_admin` is the one skill
that does it — it installs every record in the staged manifest, the two business dashboards
included, and offers to remove the retired ids those four replaced. `tests/e2e/src/manifest_roles.rs` pins the shape.
`scripts/validate-services.sh` fails CI on a plugin without a scope rule, an orphaned enabled skill
or artifact, an allow-type skill rule, two governance-hook owners, or any enabled plugin/skill/artifact
that depends on a disabled MCP server; core's `ServicesConfig::validate` refuses the last one at boot.

`source:` selects where members come from — keep it `explicit` and list ids under `include:`. Leaving it to default to `Instance` makes the plugin claim every skill and agent on the instance, which is how the marketplace once showed every plugin with all 230 skills.

Every id listed must resolve to a real top-level resource in `services/`. `ServicesConfig::validate()` enforces this at load time.

Skills follow the same nested shape: `services/skills/<id>/config.yaml`, with the instruction body beside it.
