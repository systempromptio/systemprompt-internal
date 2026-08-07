---
title: "Download for Windows"
description: "Download the Systemprompt Internal Bridge for Windows — the desktop app that connects Claude Code, Claude Desktop, and Codex CLI to the governance gateway."
author: "systemprompt.io"
slug: "download-windows"
keywords: "download, windows, exe, bridge, desktop app, install, smartscreen"
kind: "guide"
public: true
tags: ["download", "bridge", "windows"]
published_at: "2026-08-07"
updated_at: "2026-08-07"
after_reading_this:
  - "Install the bridge on Windows and launch the tray app"
  - "Get past the SmartScreen prompt on the unsigned build"
  - "Sign in and connect Claude Code through the gateway"
---

# Download for Windows

The Systemprompt Internal Bridge is a system-tray app for 64-bit Windows. It
holds your credential, syncs your organization's plugins and MCP servers into
Claude Code, Claude Desktop, and Codex CLI, and runs the local inference proxy
that routes every request through the governance gateway.

<a href="https://github.com/systempromptio/systemprompt-internal/releases/latest/download/systemprompt-internal-bridge-windows.exe" class="download-cta">Download for Windows (.exe) &rarr;</a>

## Install

1. Run the downloaded `systemprompt-internal-bridge-windows.exe`.
2. The build is currently unsigned, so SmartScreen will warn — choose
   **More info → Run anyway**.
3. A tray icon appears in your system tray.
4. Click **Sign in with systemprompt** — your browser opens against the
   gateway, and the device is linked automatically once you approve.

## Verify the download

Every release ships a `SHA256SUMS` manifest, cosign-signed keyless from the
release workflow. In PowerShell:

```powershell
(Get-FileHash .\systemprompt-internal-bridge-windows.exe -Algorithm SHA256).Hash
# compare against the matching line in:
# https://github.com/systempromptio/systemprompt-internal/releases/latest/download/SHA256SUMS
```

## Other platforms

- [macOS](/documentation/download-macos)
- [Linux](/documentation/download-linux)
- [All releases](https://github.com/systempromptio/systemprompt-internal/releases)
