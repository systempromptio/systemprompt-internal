# Astound Bridge

A branded, white-label build of the **systemprompt bridge** (credential helper +
plugin/MCP sync agent + local inference proxy) for Astound Digital. The desktop
app for Windows and macOS that connects Claude Desktop / Claude Code / Codex to
the Astound governance gateway.

This crate is intentionally tiny. All behaviour lives in the shared core library
(`systemprompt-core/bin/bridge`); here we only supply a `Brand` value and the
brand assets. The crate is a **standalone workspace** (its own `[workspace]`),
not a member of the main Astound server workspace, because it carries GUI
dependencies and ships on its own release cadence — exactly like core's bridge.

## Build & run

```bash
cd bridge
cargo build --release                 # host target
cargo run -- help                     # show Astound-branded help
cargo run -- gui                      # native settings UI (macOS/Windows only)
```

The GUI (winit + wry) compiles only on macOS/Windows; on Linux the crate builds
in headless/proxy mode.

Config, PAT, cache, and logs are isolated under the `astound` / `astound-bridge`
paths (e.g. `~/.config/astound/astound-bridge.toml`), and all env overrides use
the `ASTOUND_BRIDGE_` prefix (`ASTOUND_BRIDGE_GATEWAY_URL`, `ASTOUND_BRIDGE_PAT`,
`ASTOUND_BRIDGE_CONFIG`, …).

## macOS .app bundle

```bash
cargo build --release --target aarch64-apple-darwin
scripts/make-mac-app.sh --target aarch64-apple-darwin   # → AstoundBridge.app
```

## ⚠️ Placeholder assets — replace before release

`assets/icon.svg`, `assets/logo.svg`, and `assets/app-icon.ico` are Astound's
real marks (copied from `storage/files/images/`). But the **raster icons**
`assets/window-icon-1024.png` and `assets/tray-icon.png` are currently copies of
the systemprompt artwork (no SVG→PNG tooling was available when scaffolding).
Replace them with rendered Astound icons (1024×1024 window icon; ~44px template
tray icon) before cutting a release. `app-icon.ico` should be regenerated from
the final window icon as a multi-resolution `.ico`.

Also set `default_gateway_url` in `src/main.rs` to the real Astound gateway host
(currently a `https://gateway.astounddigital.com` placeholder).

## Recipe: a new client bridge

The core/extension boundary makes a new white-label bridge a copy-and-swap job —
no forking of the bridge source:

1. Copy this `bridge/` crate to the new repo.
2. Replace everything in `assets/` with the client's marks + `theme.css`
   (override the `--sp-*` tokens; see `assets/theme.css`).
3. Edit the `Brand` const in `src/main.rs`: name, binary name, vendor, on-disk
   dir names, `env_prefix`, `default_gateway_url`, and chrome strings. Leave
   `synthetic_plugin_name` as the shared value unless the client's gateway emits
   a matching renamed managed-plugin name (wire-contract — coordinate both
   sides).
4. Update `build.rs` (Windows metadata), `macos/Info.plist` (bundle id + names),
   and `scripts/make-mac-app.sh` (bundle/app name).
5. Wire up the release workflow (`.github/workflows/release-bridge.yml`).

Everything else — auth, sync, proxy, GUI, host integrations — is inherited from
core and stays in lockstep across all brands.
