# Changelog

All notable changes to this repository are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

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
