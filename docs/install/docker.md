# Install from the published image

Every release publishes a prebuilt, multi-arch (amd64 + arm64) image to
`ghcr.io/systempromptio/systemprompt-internal`. Installing means pulling it —
no Rust toolchain, no compile.

## 1. Access

If the package is private, you need a GitHub account with read access to
`systempromptio/systemprompt-internal` and a **personal access token (classic)**
with the `read:packages` scope
(GitHub → Settings → Developer settings → Personal access tokens).

```bash
echo "$GITHUB_PAT" | docker login ghcr.io -u <github-username> --password-stdin
```

If the pull is denied although the repo is visible to you, a maintainer must
grant your team read access on the package once: repository → Packages →
`systemprompt-internal` → Package settings → Manage access.

## 2. Configure

```bash
git clone https://github.com/systempromptio/systemprompt-internal.git   # for docker-compose.yml + .env.example
cd systemprompt-internal
cp .env.example .env
```

The shipped `docker-compose.yml` builds from source. To run the published image
instead, replace the app service's `build:` with
`image: ghcr.io/systempromptio/systemprompt-internal:<tag>`.

Edit `.env`:

| variable | required | notes |
|---|---|---|
| `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` / `GEMINI_API_KEY` | one of them | the first set becomes the default provider |
| `SYSTEMPROMPT_ADMIN_EMAIL` | yes, first boot | the administrator the container's first-boot `admin setup` creates; the entrypoint refuses to start without it |
| `ENCRYPTION_MASTER_KEY` | yes, keep it | 64 hex characters; core refuses to start without one, and stored connector grants are unreadable without the same key — see [required secrets](required-secrets.md) |
| `ODOO_URL`, `ODOO_DB` | for the Odoo connector | the Odoo instance tool calls run against |
| `EXTERNAL_URL` | on a server | the public URL, e.g. `https://gateway.example.com` — sets `api_external_url` and CORS |
| `POSTGRES_PASSWORD`, `HTTP_PORT`, `PG_PORT` | no | bundled Postgres and port mapping |
| `DATABASE_URL` | remote DB only | see below |
| `DATABASE_WRITE_URL` | read/write split only | primary for writes when `DATABASE_URL` is a read replica; core refuses a standby as write target |

## 3. Run

```bash
docker compose pull && docker compose up -d
curl -fsS http://localhost:8080/api/v1/health
```

First boot writes the docker profile, waits for Postgres, runs migrations and
bootstraps an admin. Open `http://localhost:8080/admin` to finish setup.

### Remote Postgres

Set `DATABASE_URL=postgres://user:pass@host:5432/systemprompt` in `.env` and
start only the app. The database needs the `uuid-ossp` and `pgcrypto`
extensions; pgvector is not required.

### More than one node

The `.env` path above is **single-node only**. On first boot it authors a
profile inside the container and generates that container's own signing key,
OAuth pepper, and manifest seed; a second node would generate different ones
and reject every token the first node minted. For two or more nodes (and for
any deployment where secrets come from a vault rather than a `.env` file),
render one profile directory, identical on every node, and bind-mount it with
`SYSTEMPROMPT_PROFILE_DIR`.

### Upgrade, pin, roll back

```bash
docker compose pull && docker compose up -d                  # follow latest
# pin: set the image tag in docker-compose.yml to a release, e.g. :<version>
```

Versioned tags (`X.Y.Z`, `X.Y`, `X`) and `latest` are written only after the
release's smoke and upgrade-boot proofs pass on the exact digest; every build
is also published as `sha-<commit>`. Versioned tags are never rewritten, but
old releases are pruned (the newest three are kept), so pin one you can see on
the package page. Core migrations are forward-only: roll the image back only to
a version whose migrations match the database.

**Upgrading an existing install across 0.44.0:** the provider catalog and <!-- pinned-release -->
gateway routes moved out of the profile and into the image
(`services/ai/providers.yaml` / `services/ai/gateway.yaml`), and boot now
refuses to start if the profile still has a top-level `providers:` (or
`gateway:`) section. A fresh container is unaffected — first boot writes a
clean profile — but if you persist `.systemprompt/profiles/` across upgrades,
migration fails at "Running database migrations..." with `Profile
initialization failed ... still carries a top-level providers: section`. Fix it
before restarting: open the mounted `profile.yaml`, delete the entire
`providers:` block (and `gateway:` if present), and restart — the catalog ships
baked into the image.

### Changing configuration between releases

`services/` is baked into the image and read once at boot, so a running node
serves what it was built with. To change it without waiting for the next image,
bind-mount your own copy over `/app/services` and restart — the container
already sets `SYSTEMPROMPT_SERVICES_PATH=/app/services`, so the mount is all
that is needed. Admin-UI edits to gateway routes write inside the container and
are lost when it is replaced unless that tree is mounted, and a `:ro` mount
makes the UI editor fail on write.

### Verify the image

Images built by the release pipeline are signed keylessly with cosign:

```bash
cosign verify \
  --certificate-identity-regexp='https://github.com/systempromptio/systemprompt-internal/' \
  --certificate-oidc-issuer=https://token.actions.githubusercontent.com \
  ghcr.io/systempromptio/systemprompt-internal:<version>
```

## 4. The bridge (Claude Code client)

The gateway serves its own bridge binaries at `<gateway>/files/downloads/`
(they are baked into the image), and the admin **Bridge Setup** page links
them. The same files are attached to the GitHub Release `bridge-v<version>`:

| file | platform |
|---|---|
| `systemprompt-internal-bridge-linux-x86_64.tar.gz`, `systemprompt-internal-bridge-linux-aarch64.tar.gz` | Linux (glibc ≥ 2.35) |
| `systemprompt-internal-bridge-macos.dmg` | macOS, universal (Apple Silicon + Intel) |
| `systemprompt-internal-bridge-windows.exe` | Windows x86_64 |
| `install.sh` | Linux one-liner installer |

```bash
curl -fsSL <gateway>/files/downloads/install.sh | sh -s -- \
  --download-base <gateway>/files/downloads --code <connect-code>
```

The macOS app and DMG are Developer ID signed, notarized and stapled, so
Gatekeeper opens them without a prompt (`just bridge-verify-macos <dmg>` checks
the team, notarization tickets and Gatekeeper verdict on a downloaded copy).
The Windows exe is **not OS-signed**: SmartScreen asks for *More info → Run
anyway*. Every asset is checksummed (`SHA256SUMS`) and Sigstore-signed
regardless — see the release notes for the `cosign verify-blob` line.

### Pointing a bridge at a different gateway (local instance, staging)

The bridge verifies every synced manifest against an ed25519 public key. Each
instance signs with its own key (derived from the profile's
`manifest_signing_secret_seed`, generated at `admin setup`), so a local
`just start` instance and the production gateway never share one. The bridge
learns the key on its first sync (trust-on-first-use) and stores it in
`[sync] pinned_pubkey` **together with the gateway it was learned from**. A
pin for one gateway is ignored for another, and the next sync re-learns it.
`systemprompt-internal-bridge doctor` reports which gateway the pin belongs to.

The key can also be supplied out of band, and that source always wins:

| source | where | precedence |
|---|---|---|
| env var | `SYSTEMPROMPT_BRIDGE_POLICY_PUBKEY` | highest |
| managed policy | the bridge's Windows policy key or macOS configuration profile, value `manifestPubkey` (`doctor` names the location it read) | second |
| config file | `[sync] pinned_pubkey` + `pinned_pubkey_gateway` in `systemprompt-internal-bridge.toml` | lowest |

A sync that fails with *"does not match the pubkey pinned from the policy"*
means one of the first two rows holds a key for a different gateway. Read the
current gateway's key and repin:

```bash
curl -s http://localhost:8080/v1/bridge/pubkey
systemprompt-internal-bridge install --apply --pubkey <base64 from the response>
```

### Windows: which registry hive the policy lands in

The Claude Desktop / Cowork policy (`SOFTWARE\Policies\Claude`) is written to
**HKCU** when the bridge runs as an ordinary user, and to **HKLM** when it runs
elevated (or via `install --apply`, which prompts once). Cowork honours HKCU
only while no HKLM key exists, so the bridge refuses an HKCU write that a
conflicting HKLM key would shadow and says so in the sync result. Every write
is read back; a value that did not land is reported, never assumed.
`systemprompt-internal-bridge doctor` shows which hive holds the policy.

## Building from source instead

```bash
docker compose up --build
```

This is the shipped compose path and needs the full toolchain inside the
builder stage (~25 min cold). It exists for Dockerfile work and air-gapped
hosts.
