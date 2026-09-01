# Install the Bridge desktop app (macOS)

Installs **Systemprompt Internal Bridge** — the desktop app that runs the local
inference proxy, holds your gateway credentials, and syncs plugins, skills and
MCP servers into Claude Desktop, Cowork and Codex CLI.

This is a different product from the gateway server, but it ships with it:
every merge to `main` publishes GitHub Release `bridge-v<version>` with the
same version as the gateway image, so `latest` is the build released with the
deployed gateway. The admin **Bridge Setup** page links the exact version.

## Download

One disk image covers **both Apple Silicon and Intel Macs**. There is no
separate "Apple Silicon build" to choose — the release lipos the
`aarch64-apple-darwin` and `x86_64-apple-darwin` slices into a single
`universal2` executable.

```bash
curl -LO https://github.com/systempromptio/systemprompt-internal/releases/latest/download/systemprompt-internal-bridge-macos.dmg
```

Asset names are version-less on purpose, so `releases/latest/download/<asset>`
and `releases/download/bridge-v<version>/<asset>` are permanent links.

| OS | Arch | Asset |
|---|---|---|
| macOS | Apple Silicon **and** Intel | `systemprompt-internal-bridge-macos.dmg` |
| Linux | x86_64 | `systemprompt-internal-bridge-linux-x86_64.tar.gz` |
| Linux | aarch64 | `systemprompt-internal-bridge-linux-aarch64.tar.gz` |
| Windows | x86_64 | `systemprompt-internal-bridge-windows.exe` |

Requires macOS 10.15 or later (`LSMinimumSystemVersion`).

## Install

Open the dmg and drag **Systemprompt Internal Bridge.app** to `/Applications`.

Release builds are codesigned, notarized and stapled, so Gatekeeper opens them
without a right-click override. A build produced locally, or a release cut
without Apple credentials present, is unsigned — right-click → Open the first
time.

## Verify the architecture

If you want to confirm the binary really covers your CPU:

```bash
lipo -archs "/Applications/Systemprompt Internal Bridge.app/Contents/MacOS/systemprompt-internal-bridge"
# expected: x86_64 arm64
```

Both architectures listed means it runs natively either way — no Rosetta.

## First run

Launch the app. On a machine with no stored credentials it opens to the splash
and asks for a personal access token.

Issue a token on `/admin/access/tokens` on your gateway, then either paste it
into the splash or run:

```bash
systemprompt-internal-bridge login sp-live-... --gateway https://internal.systemprompt.io
```

`--gateway` is persisted to `systemprompt-internal-bridge.toml`, alongside the
PAT path and the pinned manifest signing key. Point it at whichever gateway you
authenticate against — the pinned key and the PAT are both issued per gateway,
so they cannot be mixed between servers.

Check what is wired up:

```bash
systemprompt-internal-bridge status    # config paths + what is set up
systemprompt-internal-bridge whoami    # authenticated identity, from the gateway
```

The proxy listens on `127.0.0.1:48217`, swapping a loopback secret for a fresh
gateway JWT and refreshing in the background.

## Build it yourself

```bash
just bridge-build                              # release binary (this arch only)
bridge/scripts/make-mac-app.sh                 # .app for this arch
bridge/scripts/make-mac-app.sh --universal     # both slices, lipo'd — what CI ships
```

`--universal` needs both targets built first:

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cd bridge
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin
scripts/make-mac-app.sh --universal
```

The bundle lands at `bridge/target/release/Systemprompt Internal Bridge.app` and
prints its version and architectures on completion. Its `AppIcon.icns` is
rendered from `bridge/assets/window-icon-1024.png` — regenerate the raster icons
from the `icon.svg` master with `bridge/scripts/render-icons.py` rather than
editing them by hand.

To codesign and notarize a local build, see `bridge/scripts/sign-mac-app.sh`
(run it on the `.app` before building the dmg, then again on the finished dmg).

## Installing the hosts the Bridge pairs with

### Codex

The Bridge installs managed configuration for Codex across all three of its
surfaces — CLI, desktop app, and IDE extension — under the bundle identifier
`com.openai.codex`.

```bash
npm install -g @openai/codex     # CLI
codex app                        # desktop app: downloads and runs the installer
```

`codex app` detects the processor and fetches the matching build, so it is the
reliable install path — do **not** hand-pick a dmg from the download page. The
Codex desktop app shipped Apple-Silicon-only at first and Intel support landed
later; a manually chosen `arm64` dmg fails on an Intel Mac with "you can't open
the application … not supported on this Mac", which looks like a corrupt
download but is an architecture mismatch.

The Bridge probes for the desktop app at `/Applications/Codex.app`. Note that
some builds install with a different display name on disk while keeping the
`com.openai.codex` identifier — check the bundle identifier, not the filename,
when diagnosing a host the Bridge reports as not installed:

```bash
/usr/libexec/PlistBuddy -c "Print :CFBundleIdentifier" /Applications/Codex.app/Contents/Info.plist
lipo -archs /Applications/Codex.app/Contents/MacOS/*
```

### Gateway wire format

Codex must be pointed at the gateway's **Responses** surface. The gateway serves
`POST /v1/messages` (Anthropic) and `POST /v1/responses` (OpenAI Responses); it
does **not** serve `/v1/chat/completions`. So the provider profile needs
`wire_api = "responses"` — the `"chat"` setting that most third-party
OpenAI-compatible gateways expect will 404 here. The Bridge writes this for you
under `model_providers.systemprompt.*`; set it by hand only when configuring a
host the Bridge does not manage.
