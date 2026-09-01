---
title: "Download for Linux"
description: "Download the Systemprompt Internal Bridge for Linux — one-line installer or tarballs for x86_64 and aarch64, with checksum verification."
author: "systemprompt.io"
slug: "download-linux"
keywords: "download, linux, tarball, x86_64, aarch64, arm64, bridge, install.sh, headless"
kind: "guide"
public: true
tags: ["download", "bridge", "linux"]
published_at: "2026-08-07"
updated_at: "2026-08-20"
after_reading_this:
  - "Install the bridge with the one-line installer"
  - "Or install manually from the tarball for your architecture"
  - "Verify the checksum before installing"
---

# Download for Linux

On Linux the bridge runs headless: a loopback inference proxy takes the GUI's
place, plus systemd user units for periodic sync. The installer takes a bare
box to a working `claude` — download, checksum verification, Claude Code
install, sign-in, environment, proxy, sync.

## One-line install (recommended)

```bash
curl -fsSL https://github.com/systempromptio/systemprompt-internal/releases/latest/download/install.sh | sh
```

Signing in interactively is the default; for unattended installs pass
`--code <exchange-code>` or `--pat sp-live-...`.

## Tarballs

- **[⬇ Download for Linux x86_64 (.tar.gz)](https://github.com/systempromptio/systemprompt-internal/releases/latest/download/systemprompt-internal-bridge-linux-x86_64.tar.gz)**
- **[⬇ Download for Linux aarch64 (.tar.gz)](https://github.com/systempromptio/systemprompt-internal/releases/latest/download/systemprompt-internal-bridge-linux-aarch64.tar.gz)**

Both ship on every `bridge-v*` release, cut together with the gateway it
runs against; the admin **Bridge Setup** page links the exact release for the
gateway you are signed in to.

Each archive carries the binary and an `INSTALL.md` with the manual steps.
The binary dynamically links `libdbus-1`, `libcap`, `libgcrypt`, and
`libsystemd`:

```bash
sudo apt-get install -y libdbus-1-3 libcap2 libgcrypt20 libsystemd0   # Debian/Ubuntu
```

## Verify the download

A cosign-signed `SHA256SUMS` is published beside the assets:

```bash
curl -fsSLO https://github.com/systempromptio/systemprompt-internal/releases/latest/download/SHA256SUMS
sha256sum -c --ignore-missing SHA256SUMS
```

## Other platforms

- [macOS](/documentation/download-macos)
- [Windows](/documentation/download-windows)
