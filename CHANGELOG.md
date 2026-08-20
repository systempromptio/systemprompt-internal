# Changelog

All notable changes to this repository are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

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
