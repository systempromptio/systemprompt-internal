---
title: "Download for macOS"
description: "Download the Systemprompt Internal Bridge for macOS — the desktop app that connects Claude Code, Claude Desktop, and Codex CLI to the governance gateway."
author: "systemprompt.io"
slug: "download-macos"
keywords: "download, macos, mac, apple silicon, dmg, bridge, desktop app, install"
kind: "guide"
public: true
tags: ["download", "bridge", "macos"]
published_at: "2026-08-07"
updated_at: "2026-08-07"
after_reading_this:
  - "Install the bridge on macOS and launch the tray app"
  - "Clear the Gatekeeper quarantine flag on the unsigned build"
  - "Sign in and connect Claude Code through the gateway"
---

# Download for macOS

The Systemprompt Internal Bridge is a menu-bar app for Apple Silicon Macs. It
holds your credential, syncs your organization's plugins and MCP servers into
Claude Code, Claude Desktop, and Codex CLI, and runs the local inference proxy
that routes every request through the governance gateway.

<a href="https://github.com/systempromptio/systemprompt-internal/releases/latest/download/systemprompt-internal-bridge-macos.dmg" class="download-cta">Download for macOS (.dmg) &rarr;</a>

Requires macOS 10.15 or later, Apple Silicon.

## Install

1. Open the downloaded `.dmg` and drag **Systemprompt Internal Bridge** to
   Applications.
2. The build is currently unsigned, so clear the quarantine flag once:

   ```bash
   xattr -dr com.apple.quarantine "/Applications/Systemprompt Internal Bridge.app"
   ```

3. Launch the app. A tray icon appears in your menu bar.
4. Click **Sign in with systemprompt** — your browser opens against the
   gateway, and the device is linked automatically once you approve.

## Verify the download

Every release ships a `SHA256SUMS` manifest, cosign-signed keyless from the
release workflow:

```bash
curl -fsSLO https://github.com/systempromptio/systemprompt-internal/releases/latest/download/SHA256SUMS
shasum -a 256 -c SHA256SUMS --ignore-missing
```

## Other platforms

- [Windows](/documentation/download-windows)
- [Linux](/documentation/download-linux)
- [All releases](https://github.com/systempromptio/systemprompt-internal/releases)
