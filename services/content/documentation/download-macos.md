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
  - "Clear the Gatekeeper quarantine flag on the unsigned build"
  - "Sign in and connect Claude Code through the gateway"
---

# Download for macOS

The Systemprompt Internal Bridge is a menu-bar app for Apple Silicon Macs. It
holds your credential, syncs your organization's plugins and MCP servers into
Claude Code, Claude Desktop, and Codex CLI, and runs the local inference proxy
that routes every request through the governance gateway.

> **A macOS build is not currently hosted for download** — contact your
> administrator to get the `.dmg`. The steps below apply once you have it.

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

Verify the `.dmg` against the checksum your administrator provides:

```bash
shasum -a 256 systemprompt-internal-bridge-macos.dmg
```

## Other platforms

- [Windows](/documentation/download-windows)
- [Linux](/documentation/download-linux)
