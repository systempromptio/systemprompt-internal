---
title: "Install the Desktop Bridge"
description: "Install and sign in to Systemprompt Internal Bridge with trusted configuration on macOS, Windows, Linux, and WSL."
author: "systemprompt.io"
slug: "bridge-install"
keywords: "bridge, install, sign in, connect code, manifest key, linux, wsl, macos, windows"
kind: "guide"
public: true
tags: ["documentation", "getting-started", "bridge"]
published_at: "2026-08-19"
updated_at: "2026-09-11"
related_playbooks:
  - title: "Connect Claude Code"
    url: "/documentation/connect-claude-code"
  - title: "Connect Cowork"
    url: "/documentation/connect-cowork"
  - title: "Download for macOS"
    url: "/documentation/download-macos"
  - title: "Download for Windows"
    url: "/documentation/download-windows"
  - title: "Download for Linux"
    url: "/documentation/download-linux"
---

# Install the Desktop Bridge

Systemprompt Internal Bridge links your computer to your organization's gateway. It runs a local inference proxy and synchronizes the skills, plugins, and MCP connections available to your account.

## Choose a platform

- **macOS:** [Download for macOS](/documentation/download-macos), then connect [Claude Code](/documentation/connect-claude-code) or [Cowork](/documentation/connect-cowork).
- **Windows:** [Download for Windows](/documentation/download-windows). Use the native bridge for Claude Code or Cowork, with separate client enrolment. WSL is optional and uses a Linux bridge.
- **Linux or WSL:** follow the instructions below, then [Connect Claude Code](/documentation/connect-claude-code).

Installing the bridge, signing in, and configuring a client are separate steps. A signed-in bridge does not by itself route every application on your computer.

## Gateway and signing trust

Use the HTTPS gateway address supplied by your administrator. In shell examples, replace the example address:

```bash
GATEWAY_URL="https://gateway.example.com"
```

A fresh installation also needs the gateway's trusted manifest public key. Obtain this Base64 public key through your administrator's approved channel; it is not your password, a connect code, or a provider API key. Devices configured by IT may already have it pinned.

On macOS or Linux, pin it with:

```bash
systemprompt-internal-bridge install --gateway "$GATEWAY_URL" \
  --pubkey "ADMINISTRATOR_PROVIDED_BASE64_PUBLIC_KEY"
```

Use the executable invocation from your platform guide if `systemprompt-internal-bridge` is not on your PATH. If verification reports a key mismatch, stop and contact your administrator. Do not replace a trusted key, accept an unverified key, or disable signature verification to get past the error.

## Sign in

Open your gateway's `/admin/login` page and sign in with your passkey (see [Authentication](/documentation/authentication)). You must use an account with bridge access.

Interactive sign-in on macOS or Linux:

```bash
systemprompt-internal-bridge login --gateway "$GATEWAY_URL"
```

Approve the device link in your browser and return to the terminal. Confirm the gateway address and account before approving.

Alternatively, open **Profile** and click **Generate a connect code**. Copy the code only when ready to connect:

```bash
systemprompt-internal-bridge login --gateway "$GATEWAY_URL" --code "YOUR_CONNECT_CODE"
```

Connect codes are single-use and expire after ten minutes. If a code expires or has already been used, click **Generate a connect code** again. Opening or refreshing Profile does not generate a code. Treat it as a credential: do not share it in chat, screenshots, or support logs.

## Linux and WSL

Run these steps in your Linux terminal, as your normal user. For WSL, both Claude Code and the bridge must run inside the same distribution.

You need `curl`, `tar`, a SHA-256 utility, and a supported Linux environment. See [Download for Linux](/documentation/download-linux) for architectures and checksum instructions. Minimal distributions may also need `libdbus-1-3`, `libcap2`, `libgcrypt20`, and `libsystemd0`.

Download the installer from your gateway and inspect it before running it:

```bash
GATEWAY_URL="https://gateway.example.com"
curl -fSLo bridge-install.sh "$GATEWAY_URL/files/downloads/install.sh"
less bridge-install.sh
sh bridge-install.sh --download-base "$GATEWAY_URL/files/downloads" \
  --gateway "$GATEWAY_URL" \
  --pubkey "ADMINISTRATOR_PROVIDED_BASE64_PUBLIC_KEY"
```

The installer downloads and checks the bridge archive, attempts to install Claude Code if missing, signs you in, writes bridge configuration and shell integration, applies client settings, registers background jobs, and runs sync and diagnostics. Review its output: a warning is not proof that setup completed.

The bridge installs to `~/.local/bin` for a normal user. If your shell cannot find it:

```bash
export PATH="$HOME/.local/bin:$PATH"
systemprompt-internal-bridge --version
```

Keep that directory on your PATH in your shell configuration. Run `claude --version` to confirm Claude Code is installed, then complete [Connect Claude Code](/documentation/connect-claude-code), including its explicit client-enrolment step.

### Background services in WSL

If the installer cannot enable systemd user services, keep the proxy running in a separate WSL terminal:

```bash
systemprompt-internal-bridge proxy
```

Leave that terminal open while using Claude Code. Do not start a second proxy if one is already running. With working systemd user services, register the bridge's background jobs with `systemprompt-internal-bridge install --apply-schedule`. Use a reachable gateway hostname; Windows and WSL are separate environments.

## Sync and verify

After signing in and configuring your chosen client:

```bash
systemprompt-internal-bridge sync
systemprompt-internal-bridge whoami
systemprompt-internal-bridge status
systemprompt-internal-bridge doctor
```

Confirm that identity and gateway match your intended account. Resolve failed checks for the client you are using. Doctor also checks other installed clients; a failure for another client needs separate investigation.

Keep the desktop app or proxy service running during sessions. Background synchronization depends on successful service registration; you can always request a sync manually.

## Configuration locations

| Platform | Default bridge configuration directory |
|---|---|
| macOS | `~/Library/Application Support/systemprompt-internal/` |
| Windows | `%APPDATA%\systemprompt-internal\` |
| Linux and WSL | `~/.config/systemprompt-internal/` |

An absolute `XDG_CONFIG_HOME` override changes the base directory. Use `systemprompt-internal-bridge status` to identify your actual configuration location. These directories contain credentials as well as configuration: do not upload their contents.

## Troubleshooting

- **Sign-in succeeds but requests fail:** complete your client guide and check that the local proxy is running. Use generated settings rather than hard-coding a proxy port.
- **Unauthorized or forbidden:** check `whoami`, the gateway address, and your account's access with your administrator.
- **Signature or manifest verification fails:** stop and ask your administrator to verify the trusted key.
- **Plugins are missing:** run sync, inspect its errors, restart the client, and confirm your account is entitled to the plugin.
- **A managed policy prevents setup:** ask IT to configure the integration. Do not remove corporate sign-in requirements or bypass OS protections.

To disconnect only a client, use its guide's removal instructions. Removing a client profile is different from signing out or deleting the bridge's credentials.
