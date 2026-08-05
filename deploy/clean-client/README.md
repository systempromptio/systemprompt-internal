# Clean client

A disposable Linux container with **no Astound configuration**, for testing the
Claude Code + Astound Bridge integration as a new user experiences it.

The problem it solves: a dev machine has already been set up, so an integration
test there can pass by reading config that a real customer would not have. This
container starts from an empty `$HOME` every time.

```bash
just clean-client-build      # once
just clean-client            # throwaway shell; state dies on exit
just clean-client PERSIST=1  # keep the PAT across restarts
just clean-client-reset      # wipe the persisted state
```

Point it somewhere other than this host's gateway:

```bash
just clean-client 0 https://gateway.astounddigital.com
```

## What it does and does not contain

Baked in: Node 22, Claude Code (`@anthropic-ai/claude-code`), git, ripgrep, jq,
curl, and Playwright + Chromium (build with `--build-arg INSTALL_BROWSERS=0`
for the slim bridge-only variant — the browser layer is ~1GB). Bind-mounted
read-only at run time: `bridge/target/release/astound-bridge`, so you test the
binary you just built rather than a stale copy.

### Dev sandbox variant

`just dev-sandbox <repo>` runs this same image with the given project repo
mounted read-write at `/workspace/project` (the session starts there). HOME
stays virgin and all cleanliness assertions still apply — only the project
directory crosses into the container, which is what lets the `dev_test` skill
run `npx playwright test` against a real project through a fully governed
Claude Code.

Deliberately absent — do not add these:

| Not present | Why |
|---|---|
| `--env-file .env` | provider keys would let Claude Code bypass the gateway |
| any `$HOME` bind-mount | `~/.claude`, `~/.claude.json`, `~/.config/astound` are the exact state under test |
| a mount of this repo | `.systemprompt/profiles/` and `sessions/` are real host config |

`entrypoint.sh` enforces the first and second: it refuses to start if bridge or
Claude Code config already exists (unless `CLEAN_CLIENT_ALLOW_STATE=1`, which
`PERSIST=1` sets), and warns if `ANTHROPIC_API_KEY` is set.

## Networking

`--add-host host.docker.internal:host-gateway` lets the container reach a
gateway running in your primary WSL distro at
`http://host.docker.internal:8080`. Nothing needs publishing for that direction.

Port `8767` — the bridge's plugin-OAuth loopback — is published to
`127.0.0.1:8767` so a Windows browser can complete a redirect. The recipe skips
it when the host already holds that port, and says so; the rest of the flow
still works.

Neither `astound-bridge login` (paste a `sp-live-…` PAT) nor Claude Code's OAuth
needs a browser inside the container: open the printed URL on Windows and paste
the code back.

## Test sequence

Device certs are the supported headless credential — they renew unattended,
which a device-link grant cannot (the proxy re-authenticates per request and
would try to open a browser each time).

**Admin, on the host:**

```bash
just cli admin users create --name jdoe --email jdoe@astounddigital.com --full-name Jane-Doe
just cli admin users show jdoe                    # capture the UUID
just cli admin bridge enroll-cert --user-id <UUID> --fingerprint <sha256> --label jdoe-laptop
```

**User, in the container:**

```bash
mkdir -p ~/.config/astound
openssl req -x509 -newkey rsa:2048 -nodes -days 730 \
  -keyout ~/.config/astound/device.key \
  -out    ~/.config/astound/device.pem -subj "/CN=$(hostname)"
openssl x509 -in ~/.config/astound/device.pem -outform der | sha256sum   # send to admin

# Name the cert in the config. On Linux cert_keystore_ref is the certificate's
# path; ASTOUND_BRIDGE_DEVICE_CERT still works and takes precedence.
printf '\n[mtls]\ncert_keystore_ref = "%s/.config/astound/device.pem"\n' "$HOME" \
  >> ~/.config/astound/astound-bridge.toml

astound-bridge whoami                 # your own identity, no PAT
astound-bridge install --apply --apply-schedule
astound-bridge sync --allow-tofu      # plugins, skills, agents, MCP + marketplace
astound-bridge doctor                 # names any remaining problem
astound-bridge proxy &                # no systemd here — start the proxy by hand
bash -l -c claude                     # org skills load with zero manual exports
```

`install --apply` is not optional: `sync` fails without the org-plugins directory
it provisions. `sync` also registers the `org-provisioned` marketplace with the
Claude Code CLI, so `claude plugin list` shows the plugins and skills resolve as
`<plugin-id>:<skill>` — no `--plugin-dir` needed.

`~/work` is empty; `git clone` something into it if the session needs a project.

## What `install --apply` writes, and what this container cannot prove

On Linux the bridge builds headless — `gui` is macOS/Windows only — so the
loopback `proxy` replaces it. `install --apply` writes the environment a login
shell needs:

| What | Where |
|---|---|
| `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN` | `~/.config/astound/env.sh` |
| a marker-delimited block sourcing that file | `~/.profile` |

That part is fully exercised here: `bash -l` then `claude` must work with no
exports, and running install twice must leave exactly one block in `~/.profile`.

`--apply-schedule` additionally writes two systemd **user** units —
`astound-bridge-sync.{service,timer}` and `astound-bridge-proxy.service`. **This
container has no systemd**, so only half of that is testable here: unit
*generation*, and the graceful degradation (files written, a warning printed,
exit 0). Activation and restart-on-failure must be confirmed on a real systemd
host — a second WSL distro, or the host itself.

## Alternative: the one-line installer

Instead of mounting the binary, point the container at a running gateway and
install the published artefact exactly as a customer would:

```bash
curl -fsSL http://host.docker.internal:8080/files/downloads/install.sh | sh -s -- \
  --download-base http://host.docker.internal:8080/files/downloads
```

It verifies the tarball checksum and refuses to proceed on a mismatch — corrupt
a byte of the published tarball to prove that path.

## Runtime dependencies this container proved

The branded binary dynamically links `libdbus-1.so.3` (via keyring-core's
`dbus-secret-service-keyring-store`), plus `libsystemd`, `libcap`, `libgcrypt`.
A stock slim Debian lacks `libdbus-1-3`, and without it the binary cannot even
print its help. **Any Linux tarball we publish carries the same dependency** —
either document it, or drop the secret-service store on Linux and fall back to
the 0600 PAT file.

Keyring operations want a session bus, which no container has by default. That
is no longer a blocker: with neither a Secret Service provider nor a bus, the
bridge tiers down to the kernel keyutils keyring, so no command here needs
wrapping.

`gnome-keyring` is deliberately **not** installed. A container has no Secret
Service provider, which is exactly the condition a real headless Linux user hits;
the bridge tiers down to the kernel keyutils keyring (and then to process memory)
and reports which one it chose via `astound-bridge doctor`. Installing a desktop
keyring here would hide that path instead of testing it.
