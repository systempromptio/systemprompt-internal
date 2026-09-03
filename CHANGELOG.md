# Changelog

All notable changes to this repository are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Changed

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
