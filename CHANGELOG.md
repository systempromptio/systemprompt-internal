# Changelog

All notable changes to this repository are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.36.0] - 2026-08-24

### Fixed

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
