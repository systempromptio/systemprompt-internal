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
updated_at: "2026-08-20"
after_reading_this:
  - "Install the bridge on macOS and launch the tray app"
  - "Verify the signed, notarized build before installing"
  - "Sign in and connect Claude Code through the gateway"
---

# Download for macOS

The Systemprompt Internal Bridge is a menu-bar app for Apple Silicon and Intel
Macs (one universal build). It
holds your credential, syncs your organization's plugins and MCP servers into
Claude Code, Claude Desktop, and Codex CLI, and runs the local inference proxy
that routes every request through the governance gateway.

**[⬇ Download for macOS (.dmg)](https://github.com/systempromptio/systemprompt-internal/releases/latest/download/systemprompt-internal-bridge-macos.dmg)**

Requires macOS 10.15 or later.

## Install

1. Open the downloaded `.dmg` and drag **Systemprompt Internal Bridge** to
   Applications.
2. The app is signed with our Developer ID, notarized by Apple and stapled,
   so it opens without a quarantine prompt.
3. Launch the app. A tray icon appears in your menu bar.
4. Click **Sign in with systemprompt** — your browser opens against the
   gateway, and the device is linked automatically once you approve.

## Verify the download

Check Apple's signature, or the cosign-signed `SHA256SUMS` beside the asset:

```bash
spctl --assess --type open --context context:primary-signature -vv systemprompt-internal-bridge-macos.dmg
curl -fsSLO https://github.com/systempromptio/systemprompt-internal/releases/latest/download/SHA256SUMS
shasum -a 256 -c --ignore-missing SHA256SUMS
```

## Other platforms

- [Windows](/documentation/download-windows)
- [Linux](/documentation/download-linux)
