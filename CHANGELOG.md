# Changelog

All notable changes to this repository are recorded here, newest first.

Conventions (strict — hold every entry to them):

- Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/): an `## Unreleased`
  section at the top, then one `## [X.Y.Z] - YYYY-MM-DD` section per release, each with only
  the categories it needs, in this order: `### Breaking`, `### Added`, `### Changed`,
  `### Fixed`, `### Removed` (plus `### Migration` where an operator must act).
- Entries are written for the reader who did not make the change: full sentences, what changed
  and **why**, named files/commands/flags where the reader will need them. No bare "updated X".
- Every user-visible or operator-visible change lands in `Unreleased` **in the same commit** as
  the change itself; internal-only refactors are recorded when they alter an API another crate,
  config, workflow or dashboard consumes.
- A release moves the `Unreleased` content under its version heading; `Unreleased` is never
  deleted, only emptied.
- Versions are lockstep with core: `X.Y.Z` is the workspace `version`, the core it builds
  against, the `vX.Y.Z` gateway release and the `bridge-vX.Y.Z` desktop release.

## Unreleased

### Added

- **Release:** frozen promotion. `just release X.Y.Z` (`scripts/release.sh`) promotes the
  exact `next` commit whose push-triggered Gates run is green: it pushes
  `promote/X.Y.Z/<main>/<next>`, opens the PR, and on a second run merges only after the PR's
  own proof and the merge tree are verified. `release.yml` re-verifies the merge
  (`scripts/check-release-merge.sh`) before publishing. Replaces `just gate` / `just promote`
  and the mutable `promote` ref. Mocked self-tests live in `tests/scripts/`.
- **CI:** `.github/workflows/gates.yml` replaces `ci.yml` and `quality.yml` and runs on every
  push to `next` (previously nothing ran on a push). Independent tiers — static, lint, test
  (now including the e2e suite), bridge, supply chain — and one `Gates passed` aggregate for
  the `main` ruleset.
- **Release:** the image is proved before it is tagged. `docker.yml` publishes only a signed
  `:sha-<short>`; `release.yml`'s `smoke` (version, MCP binaries, arches, signature) and
  `upgrade-boot` (the image's entrypoint over every seeded release schema, rows must survive)
  gate `promote-tags`, which alone moves `:X.Y.Z`, `:X.Y`, `:X` and `:latest`.
- **Coverage:** `.github/workflows/coverage.yml` measures the floor and ratchet on `main`,
  nightly and on demand; `scripts/coverage-badge.sh` renders and checks the README badge.
- **Deploy:** `just deploy-release X.Y.Z` (`scripts/deploy-release.sh`) ships a published
  release from a clean `origin/main` worktree with its own tarball binaries and watches Fly and
  `/health` for five minutes. `just fetch-release` installs release binaries without a
  toolchain (Linux, macOS arm64).
- **Build:** the coordinator refuses to start a run when the target volume has less than
  `BUILD_MIN_FREE_GB` (default 25) GB free — a full disk from another session's build is how
  the host went down on 2026-09-28.
- **Gates:** fifteen source gates ported from astound (discarded results, fail-open guards,
  crate layering, repository construction, JSON values, silent test skips, field copies,
  Dockerfile paths, dropped schema, docs version, core ref, schema ladder, coverage badge,
  template fields; `check-migration-numbers` is wired in). Known debt is listed explicitly and
  fails when stale.
- **Just:** `core-pin`, `core-guard` (deploy refuses a dirty or unpinned core while patched),
  `schema-baseline`, `stop`, `hack`, `lint-silent-skips`, `lint-no-untyped-admin`,
  `coverage-badge`, `test-e2e`.
- **Schema:** declarative web schemas for the sync state, service sources, marketplace
  versions, conversation analyses and facts, request scopes (a statement trigger on
  `ai_requests` stamps each request's group and project at insert), time-bound access,
  gateway routes and the staged governance chain, tool artifacts, user last-seen, raw-evidence
  expiry and the retention ledger (`extensions/web/schema/32`–`48`), each table with one twin
  migration at slots 085–094. Migration 089 backfills `ai_request_scopes` for every existing
  request. The console, jobs and rollups that fill these tables land in the next stage.
- **Access control:** `services/access-control/rules.yaml`, the entity-centric declaration
  of every entitlement (`entity`, required `why`, `default`, `owner`, `valid_until`,
  `allow`/`deny` bands), converted entry for entry from `roles.yaml`. `slack_channel` joins
  `gateway_route` and `hook` as a glob-only kind. `/documentation/access-control` and
  `/documentation/services-sync` describe the model.
- **Kits:** `deploy/kit/` (the kit repository template, rebranded; `known-kits.json` empty),
  `just services-pin <kit> <digest|channel>` (`scripts/services-pin.py`) and
  `just kit-export <marketplace> <dir>` backed by the new `extensions/cli/kit-export` crate.
  `docs/kits-on-another-instance.md` and the generic parts of `docs/CONFIGURED-CONNECTORS.md`.
- **Scheduler:** core jobs `managed_inventory_refresh` (also run at boot), `oauth_cleanup`,
  `user_rate_limit_prune`, `thought_signature_cleanup` and `otlp_export`.
- **Providers:** `claude-opus-5-5`; `max_thinking_budget` on Cerebras `gpt-oss-120b`, so its
  reasoning cannot starve a short answer.

### Changed

- **Build:** the coordinator no longer skips a recipe as "already green" — every run compiles
  the current tree; `BUILD_FORCE` is a no-op.
- **Just:** `just test` runs every tier and reports every failure; tiers use
  `--no-fail-fast`, DB tiers fail without a database URL, and the contract tier sets
  `SYSTEMPROMPT_REQUIRE_DB=1`. `preflight-static`/`preflight-lint` collect failures;
  `preflight` adds tests and coverage. `core-checkout` never moves the core checkout and fails
  (or warns, `MISMATCH=warn`) when it differs from `bridge/CORE_REF`. `machete`, `deny` and
  `audit` scan all three workspaces. `just clippy` also lints the tests workspace and the
  bridge's Windows cfg set on Linux.
- **Versions:** `scripts/sync-core-version.sh` owns the core pins and `bridge/CORE_REF`;
  `scripts/sync-release-version.sh` calls it at the same (lockstep) version.
- **Image:** the Dockerfile caches the toolchain in its own layer, gains an `artifacts`
  stage, and creates `/app/storage/data` so a fresh named volume is writable by the app user.
  `.dockerignore` keeps `*.pem` and build residue out of the context.
- **Governance:** the policy chain runs in warn mode (`governance.mode: warn`): every stage
  still runs and audits, a confirmed match is recorded as `decision=warn`, and nothing refuses
  — except `require_approval`, which names `mode: enforce`. The refused-path demo shows warning
  rows until a stage is put back to enforce. `secret_scan.patterns` is an explicit
  35-signature catalogue; a signature not listed is not scanned for.
- **Gateway:** safety scanning is on in warn mode (`[heuristic, secrets, pii_extended]`,
  pinned heuristic phrases, `history: off`) with a warn-mode per-user hourly quota;
  `services/ai/gateway.yaml` gains route names and descriptions,
  `default_model: "claude-sonnet-5[1m]"` and `quota_fault_mode: closed`.
- **MCP:** every server declares `tool_policy: allow`.
- **Services:** the enterprise-demo marketplace config carries no `access:` block;
  `scripts/validate-services.sh` validates `rules.yaml` and requires it to agree with
  `roles.yaml`, which the server still reads until the `rules.yaml` loader lands.
- **Docs:** `docs/profile.schema.json` matches core 0.61's profile (`services`, `judge`,
  `observability`, `retention`, `storage`; no `gateway`/`providers`). `docs/install/binary.md`
  and `nix.md` describe this repository's own release instead of template v0.2.2, and
  `check-docs-version` now enforces their version.
- **Gates:** `lint-schema.sh` ignores dollar-quoted function bodies and exempts
  `schema/retire/`.

### Fixed

- The access-control page logs a failed open-entity count instead of silently rendering zero.
- `odoo_identity` is declared again (`schema/15_odoo_identity.sql`, migration 083): its
  schema file was deleted in 2f9efe57 while the Odoo MCP server still read and wrote the
  table, so a database installed since had nowhere to keep per-user Odoo credentials.

### Removed

- `.github/workflows/ci.yml`, `.github/workflows/quality.yml`, the `gate`/`promote` recipes and
  the disabled pre-push hook.
- `email_outbox` and the four `comms_*` tables, left behind by the deleted email and comms MCP
  servers (migration 084). The dropped-schema gate's known-debt list is now empty.

## [0.50.0] - 2026-09-10

### Changed

- Adopted systemprompt core 0.50.0 from crates.io; the `[patch.crates-io]`
  blocks in `Cargo.toml` and `tests/Cargo.toml` stay dormant,
  `bridge/CORE_REF` pins core `568f36172`, the `v0.50.0` commit, and all three lockfiles are
  re-resolved.
- **Breaking (core):** `SecretsBootstrap::init` and `try_init` are `async`.
  The four extension binaries that bootstrap their own process — the
  systemprompt, Odoo and knowledge-bank MCP servers and the `dev-login` CLI —
  await them.
- **Breaking (core):** `AppPaths::from_profile` takes a services-root
  override. Every call site here passes `None`: this deployment composes no
  services bundles and keeps the baked tree.
- **Breaking (core):** `IngestOptions` carries `source` and `scope`, and
  `UpsertRuleParams` carries `source`. Rules written from the admin dashboard
  are stamped `DASHBOARD_SOURCE` so ingestion never overwrites or prunes an
  operator edit; the YAML loaders under `extensions/web/admin` stamp
  `YAML_SOURCE` and declare no scope, being the only writer of `yaml` rows.
- The services tree, secrets source and gateway quota fault mode keep their
  pre-0.50 behaviour: `services.sources` stays empty, `secrets.source` stays
  `file`, and `gateway.quota_fault_mode` is left at its `open` default.
  Adopting signed bundles or Vault is a separate, deliberate change.
- Outbound HTTP that a caller can influence now runs on core's guarded client,
  which re-checks every resolved address against the SSRF block list on the
  first request and on each redirect hop. The governance authz hook, MCP
  transport, OAuth metadata fetches, agent webhooks and Slack `response_url`
  replies are covered by that change in core.
- `governance_decisions` is append-only from core migration 018: an `UPDATE`
  is refused by trigger. Nothing here updates a recorded decision.

### Migration

Production runs 0.47.0 and jumps to 0.50.0 in one step. This repository cut
`v0.48.0` and `v0.49.0` but deployed neither, and no `[0.48.0]` section was
ever written here, so an operator takes the 0.49.0 notes as well as these.

- The 0.49.0 fail-closed changes land at the same time as this one: a gateway
  request whose entitlements, spend cap or subject attributes cannot be read
  is now refused with a transient `Unavailable` denial rather than allowed.
  Expect refusals, not silent over-grants, during a database blip.
- Core migrations 017 (`access_control_rules.source`), 018
  (`governance_decisions` append-only) and 022
  (`ai_requests.upstream_latency_ms`) run on first boot. 017 backfills
  existing rows to the YAML source, so any rule an operator authored in the
  dashboard before this release is claimable by a YAML pass until it is
  re-saved.
- No profile change is required. `services.sources`, `secrets.source: vault`
  and `gateway.quota_fault_mode` are all opt-in and default to the 0.47.0
  behaviour.

## [0.49.0] - 2026-09-09

### Changed

- Adopted systemprompt core 0.49.0 from crates.io; the `[patch.crates-io]`
  blocks in `Cargo.toml` and `tests/Cargo.toml` stay dormant,
  `bridge/CORE_REF` pins the `v0.49.0` commit, and all three lockfiles are
  re-resolved.
- A gateway request whose entitlements cannot be looked up is refused. It was
  allowed: the route entity, its access rules, the caller's roles and their
  subject attributes each read as a grant when absent, so a database error at
  any of the four served a customer a model tier their plan does not include.
  The refusal is an `Unavailable` denial — transient and retryable, not a
  verdict about the plan — using the deny kind core 0.49.0 adds.
- A gateway request is also refused when the organization's spend cap cannot
  be read, rather than allowed. The cap is enforced one request late by
  design, because a request's cost is known only once it has run — but a
  lookup that keeps failing overshoots a contract cap without bound. The
  refusal is transient and clears as soon as the read recovers.
- The five subject-attribute providers behind the access matrix — group,
  project, department, organization and Salesforce — report a failed lookup
  instead of resolving the dimension as empty. A lookup that failed used to
  yield "this user holds no values for that dimension", which stops every deny
  rule keyed on that dimension from matching: the request was then allowed on
  the strength of a database error. Core 0.49.0 made
  `SubjectAttributeProvider::values_for` fallible to remove exactly that, so
  governance resolution, the gateway catalogue, the marketplace filter, the
  effective-permissions view and the access matrix now surface the error.
- `scripts/sync-release-version.sh` also rewrites the test workspace's
  `systemprompt-models` pin. It rewrote only the `systemprompt` and
  `-security` pins there, so that one crate silently stayed a release behind.

## [0.47.0] - 2026-09-06

### Changed

- Adopted systemprompt core 0.47.0 from crates.io; the `[patch.crates-io]`
  blocks in `Cargo.toml` and `tests/Cargo.toml` stay dormant,
  `bridge/CORE_REF` pins the `v0.47.0` tag, and all three lockfiles are
  re-resolved.
- The Cerebras `gpt-oss-120b` entry declares `cache_read_per_million: 0.0`.
  Core 0.47.0 excludes cached prompt tokens from `input_tokens` and refuses to
  boot a gateway route that can dispatch a token-billed model with no declared
  cache-read rate. Three of the four routes in `services/ai/gateway.yaml`
  reach this model, so an absent rate is now a boot failure rather than a
  silently free cached slice; Cerebras bills none, and the zero says so. The
  Anthropic catalogue already declared its rates, and no route reaches the
  OpenAI entries.

## [0.46.0] - 2026-09-04

### Added

- An approvals dashboard in the admin console, and the tools behind it: the
  decided half of the approvals queue is readable, rendered inside the admin
  shell rather than as a bare page.
- Demo dashboards for skills, MCP tools and the session logbook, seeded from
  the real hook and gateway wire, and since consolidated into four artifacts
  that act rather than only report. All four routes are recorded in the admin
  HTTP contract baseline.
- Skill and MCP tool usage is attributed to the invocation that caused it. The
  hook records its `plugin_id` and a mismatched one is refused, so a plugin
  cannot claim another's usage; the attribution window is covered by tests.
- Odoo gains sales, quote and partner-write tooling, and answers with typed
  rows instead of prose so callers parse a shape rather than a sentence.
- The sales pipeline groups by owner and applies stage moves before saving
  them, so a reordering is never half-written.

### Changed

- Adopted systemprompt core 0.46.0 from crates.io; the `[patch.crates-io]`
  blocks in `Cargo.toml` and `tests/Cargo.toml` are dormant again and
  `bridge/CORE_REF` pins the `v0.46.0` tag.
- The email and factsheet extensions are removed, and the build follows them
  out.
- The four Odoo files over the 300-line ceiling are split along their own
  seams; behaviour is unchanged.
- Artifacts Cowork renders in chat are compressed, so a dense dashboard no
  longer arrives as an unreadable wall.

### Fixed

- The governance entropy backstop no longer denies every request from a Mac.
- Anonymous visitors are kept out of the admin user directory.
- Real tool verdicts are separated from server authorization by shape, and the
  verdict predicate is pinned into all three decision queries — an allowed
  verdict is audited like any other.
- Skill invocations are counted from the signal clients actually send, rather
  than one they never emit.
- The unit and integration test workspaces compile again, and a Skill fixture
  builds an invocation the product recognises rather than a shape it ignores.
- The clippy, rustdoc and lint-gate debt standing on `next` is cleared.

## [0.45.0] - 2026-09-04

### Changed

- The governance and skill-usage templates meet the front-end standards: the two
  inline `style` attributes are gone, `.enforcement-section__grid` has a real
  rule (and stacks under 900px), the ad-hoc `.num` class reuses the existing
  `.numeric` table convention, and `.row-muted`/`.text-success` join the
  utilities beside `.text-danger`.
- `cargo-audit` runs from the prebuilt binary (`taiki-e/install-action`, the same
  path `cargo-deny` already takes) instead of `rustsec/audit-check`, which builds
  the tool from source on every run — a broken `tinyvec 1.13.0` release failed
  the job with nothing wrong in this tree.
- `systemprompt-web-admin` builds on its own again. Its SSR handlers reference
  items gated behind `governance-ssr`, but only the workspace root enabled that
  feature, so `cargo <cmd> -p systemprompt-web-admin` — which CI runs as its own
  step — could not compile the crate at all while the whole-workspace build was
  green. The feature is on by default in the crate now; it stays a feature so the
  files can remain identical to the template fork, which compiles the queries out.
- The admin HTTP contract baseline records `/admin/entities/skills`,
  `/admin/governance` and `/admin/governance/decisions`, which were added with
  the governance dashboards but never recorded. All three answer the same way as
  every other admin route — anonymous 307 to login, non-admin 303, admin 200.
- Knowledge-bank is admin-only again. `roles.yaml` had been changed to grant the
  MCP server to `[user]` with `default_included: true`, and the manifest test
  changed to match, but no user-scoped plugin ships the server — a manifest
  carries a server only if a plugin the holder has ships it — so the grant
  described access no user could exercise and the suite went red. The grant is
  back to `[admin]` / `default_included: false`, reaching no signed manifest by
  default, and the test asserts that of both manifests again. The in-process
  read filter and the `require_admin` checks on the write and proposal tools are
  untouched; they sit behind the grant rather than in place of it.
- `bridge/CORE_REF` pins core `d975063842910a5bbc460d72c0cb9ef94b0c5d4d` (core
  `next`, workspace version 0.45.0) rather than the `v0.45.0` tag, so the
  0.45.0 bridge carries the macOS fixes that landed after that tag was cut: the
  entropy backstop no longer denying `$TMPDIR`-shaped paths (which failed every
  Claude Code request from an affected Mac), managed MCP servers that can
  authenticate on macOS, connectors that actually sync there, and org-plugins
  provisioning that no longer fails closed on a missing directory. The pinned
  commit's own version is 0.45.0, so the release workflow's core-of-the-pinned-
  version assertion holds.

### Added

- `.github/workflows/release.yml` publishes on every merge to `main`: the
  desktop bridge for macOS (signed + notarized), Windows and Linux as GitHub
  Release `bridge-v<version>`, and the container image
  `ghcr.io/systempromptio/systemprompt-internal:<version>` (`docker.yml`,
  multi-arch, cosign-signed), and the gateway server binaries (`linux-amd64`,
  `linux-arm64`, `darwin-arm64` tarballs) as GitHub Release `v<version>`.
  Nothing publishes unless CI and Quality pass on the merge commit and
  `bridge/CORE_REF` names a core commit of the pinned version.
- `ghcr-prune.yml` + `scripts/prune-releases.sh`: retention for images
  (newest 3 versions, `sha-*`/untagged after 4 weeks) and releases (newest 3
  `v*` and 3 `bridge-v*` — one of each per core release — plus orphan tags).
- `scripts/check-release-version.sh` lint gate: the bridge carries the
  workspace version; on `main` every pin is checked by
  `sync-release-version.sh`, which now also owns `bridge/Cargo.toml` and
  `bridge/CORE_REF` (`v<version>`).

### Changed

- The bridge is versioned with core and the gateway — one number for the
  workspace, the core pin, the bridge, the release tag and the image tag.
  `next` is synced to `0.42.0` (workspace, bridge, chart, deploy pins);
  bridge `0.1.10 → 0.42.0` also clears core's `MIN_BRIDGE_VERSION` floor
  (`0.28.0`) that every branded heartbeat tripped.
- The admin Bridge Setup page, the profile connect snippet and the docs link
  the GitHub release matching the running gateway's version
  (`releases/download/bridge-v<version>/…`) for all four platforms, instead
  of a same-origin `/files/downloads` staged by `just deploy` (Windows and
  Linux only, no version). `build-all` no longer builds the bridge;
  `package-bridge-*.sh` write to `dist/` for local use only.
- The in-app self-updater is enabled via `gateway.bridge_releases` in the
  production profile (no pin: `main` is the only publisher).

### Security

- `POST /hooks/govern` no longer lets the hook body's `agent_id` raise the
  caller's access scope. The value is a self-report (Claude Code's subagent
  id), and looking it up against `services/agents/*.yaml` handed a user-scoped
  token the admin tier — waiving the tool blocklist and the approval hold —
  whenever it named an admin-scoped agent. Scope now comes from the token and
  the user's stored roles only. Calls that relied on the escalation are denied
  as their real scope dictates.

### Changed

- Governance audit rows record who acted and through what: the hook's
  self-reported agent id is kept in `evaluated_rules` under
  `principal.claimed` and shown on the audit detail page as "Agent (claimed)";
  the `agent_id` identity column holds only credential-derived identity. The
  `/govern/authz` handler records the enforcement surface (`actor_kind = mcp`
  for MCP tool calls), the verified delegate, the caller's access scope and
  the OAuth `client_id` instead of a bare `user` actor with null agent columns.
- Bridge rebuilt against systemprompt-core 0.38.0 (`bridge/CORE_REF` bumped to
  the v0.38.0 release commit) and republished as `bridge-v0.18.0`.
- Adopted systemprompt core 0.38.0 from crates.io (typed marketplace keep-sets,
  the `keep_sets` authz resolver with bulk entity loading, `SubjectRef`/`DeviceId`
  identifiers, fallible `ContextId` construction, and the
  `ai_gateway_policies.priority` / `users.name`-uniqueness migrations). The
  local-core `[patch.crates-io]` blocks are dormant again.
- Plan and ACL YAML loaders now validate the whole document before writing:
  a grant naming an entity with no catalog row is an error instead of minting a
  phantom catalog entry. Two inert grants in `plans.yaml` that named a
  nonexistent `systemprompt-admin` marketplace were removed; admin-console
  access continues to ride the `roles.yaml` admin gating.
- Governance SSR repositories are gated behind a new `governance-ssr` cargo
  feature (default off in this fork); `just prepare` builds with it so their
  query cache survives.
- Budget at-risk thresholds unified in one `BudgetState` shared by the internal
  report and enterprise console pages.
- Odoo sign-in now mints OAuth authorization codes through core's
  `mint_authorization_code` instead of a hand-mirrored copy; extension authz
  precedences derive from core's exported constants.
- Federated and passkey users get their human-readable name in `users.name`
  (core 0.38.0 dropped the uniqueness constraint that forced the email
  workaround); `display_name` is unchanged.
- Polymorphic entity/subject references across admin repositories and handlers
  now use typed ids (`EntityRef`, `SubjectRef`, `DeviceId`, `MarketplaceId`,
  `UserId`) instead of raw strings; JSON/template output is unchanged.

- The marketplace filter now delegates its candidate shrinking to core:
  `apply_keep_sets` and its hand-rolled artifact-ownership pruning are replaced
  by `MarketplaceCandidate::retain_entries`, and the local `entity_ref_for`
  mapping by `EntityRef::from_kind_and_id`. Behaviour is unchanged; the
  duplicated artifact rule now lives in one place (core) and gains core's
  per-drop tracing. Requires the next `systemprompt` core release.

## [0.36.0] - 2026-08-24

### Fixed

- **Three pieces of configuration were read from process-global state, and the
  tests that varied them could only be correct one-per-process.** `cargo test`
  threads them together, where they raced and produced fourteen failures that
  were not real -- a trap that reads exactly like a regression. Each value is now
  passed in rather than looked up globally:
  - The MCP CLI's binary and working directory come from a `CliLocation` resolved
    once at the composition root, replacing the `SYSTEMPROMPT_CLI_PATH` and
    `SYSTEMPROMPT_WORKDIR` environment reads. Neither was a sanctioned
    environment variable, and nothing outside the tests ever set them.
  - The content-ingestion job takes `delete_orphans` as a job parameter instead
    of reading `CONTENT_INGESTION_DELETE_ORPHANS`, and resolves its blog config
    from the job context's own `AppPaths` instead of the process-wide
    `BlogConfigValidated::cached()`, whose `OnceLock` fixed the answer for the
    whole process the first time any caller asked.
  - The subject-dimension registry is cached per database rather than in a single
    `OnceLock`. The providers close over the pool they were built with, so one
    process-wide registry answered every later caller from whichever database
    asked first.

  The suite now passes under `cargo test` as well as `cargo nextest`: 1227 tests,
  no failures, where fourteen failed before.
- Two shipped front-end sources carried explanatory `//` comments, which the
  front-end standards test bans outright — 55 of the other 57 files carry none,
  and the exemptions file is explicitly not for muting a fixable violation. The
  knowledge moved into names instead of being deleted: `ARTIFACTS` is
  `HOSTED_ARTIFACTS`, `REDIRECT_URI` is `REGISTERED_REDIRECT_URI`, and the OAuth
  authorize branch tests `thirdPartyClientAwaitingItsOwnCode`. The test had been
  failing since both files landed on 2026-08-20, hidden behind the contract
  failure that aborted the run before it.
- **The admin contract suite never had the `marketplace-admin` OAuth client.**
  Seeds run on every boot and the owner-dependent ones select the first admin
  user, inserting nothing when there is none -- `oauth_clients.owner_user_id` is
  NOT NULL. A real deployment installs its schema, creates an admin, then serves
  from the next boot, by which point the seed has applied; `TempDb` installs once
  and never boots again, so the client never existed and
  `signing_in_never_provisions_a_user_from_an_unproven_credential` was answered
  `400 Unknown OAuth client` instead of reaching the credential check it asserts
  on. The fixture now re-applies seeds once an admin exists, which is what the
  next boot does. Production was never affected.
- The same test posted `http://localhost/admin/login`, which is not one of the
  client's registered redirect URIs, so a well-formed request was refused at
  redirect validation before the credential was ever examined. It uses the
  registered `http://localhost:8080/admin/login`, so the refusal it asserts is
  the one it means.

Tracks systemprompt-core 0.36.0. Helm chart 0.13.0 with appVersion 0.36.0. Pin-only:
the breaking `McpDomainError::PortHolderUnverifiable` variant is not matched in this
repo, and the messaging and Slack APIs 0.36.0 changed are not used here.

## [0.35.0] - 2026-08-23

Tracks systemprompt-core 0.35.0, taking the 0.34.0 governance change this repo
had skipped.

### Fixed

- **The trace explorer joined governance rows on the wrong column.** Enforcement
  sites with no session wrote their trace id into `governance_decisions.session_id`,
  and the trace list, trace stats, and id resolver all joined against that. Core
  0.34.0 gave the table a real `trace_id`, so the webhook now writes the correlator
  to its own column, the two trace queries join `t.trace_id = g.trace_id`, and a
  session id that merely looked like a trace id can no longer pull in unrelated
  rows. Empty session ids are treated as absent rather than as a session.
- `governance` id resolution searches `governance_decisions.trace_id` as well as
  `ai_requests`, so a trace belonging to an enforcement site that issued no AI
  request resolves instead of coming back empty.

### Changed

- Bridge 0.1.10: rebuilt against core 0.32's `SignedManifestEnvelope` manifest
  wire format — bridges ≤ 0.1.9 fail every sync against a 0.32 gateway with
  "malformed manifest response" and must be reinstalled from the website.
- The website is now the bridge download source of truth. `just deploy`
  (`build-all`) packages the Linux x86_64 tarball **and** the Windows exe into
  `storage/files/downloads/`, served at `/files/downloads`; the admin Bridge
  Setup page, the documentation download pages, and `install.sh` all point
  there instead of GitHub Releases. `install.sh` is templated at publish time
  (`@DOWNLOAD_BASE@`), so the piped one-liner needs no `--download-base`.
- macOS and Linux aarch64 builds are not hosted (they require mac/ARM
  builders); the pages say so instead of linking dead or stale assets.
- The in-app self-updater stays disabled (`gateway.bridge_releases` unset —
  the feed only supports a GitHub backend); the website is the distribution
  channel.

## [0.21.0] - 2026-08-07

### Added

- Public release of the Systemprompt Internal workspace: web extensions (admin console, public site, content pipeline), MCP extensions (Odoo, knowledge bank, systemprompt), and the desktop bridge.
- Odoo MCP extension with 14 tools covering CRM, projects, and a persistent full-text knowledge store.
- Bridge release pipeline: `bridge-v*` tags build Linux (x86_64, aarch64), macOS (ARM), and Windows binaries, cosign-signed with a `SHA256SUMS` manifest.
- MIT license.

### Changed

- Dark-only theme across the admin console and public site.
- Bridge defaults its gateway URL to the production endpoint; override with `--gateway`.

### Fixed

- Odoo companion database initializes without demo data.
