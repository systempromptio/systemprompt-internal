# Install the server binary (from GitHub Releases)

Installs the Systemprompt Internal server — `systemprompt` and its
`systemprompt-mcp-*` servers — from the signed release tarballs. The desktop
bridge is a separate release series (`bridge-v0.62.0`); see
[bridge-macos.md](bridge-macos.md).

Each release `v0.62.0` publishes one tarball per platform, a `SHA256SUMS`, and
a cosign keyless signature (`.sig` + `.pem`) for every file.

| OS | Arch | Asset |
|---|---|---|
| Linux | x86_64 | `systemprompt-internal-0.62.0-linux-amd64.tar.gz` |
| Linux | arm64 | `systemprompt-internal-0.62.0-linux-arm64.tar.gz` |
| macOS | Apple Silicon | `systemprompt-internal-0.62.0-darwin-arm64.tar.gz` |

There is no Intel macOS or Windows server build; use the container image
(`ghcr.io/systempromptio/systemprompt-internal:0.62.0`, see [ghcr.md](ghcr.md))
there.

## From a checkout

`just fetch-release` downloads the tarball for this host and the workspace
version, checks it against `SHA256SUMS`, and installs every binary into
`target/release/` — no toolchain needed:

```bash
just fetch-release          # the version in Cargo.toml
just fetch-release 0.62.0   # a specific release
```

## Manual download

The repository is private, so download with an authenticated `gh`:

```bash
gh release download v0.62.0 -R systempromptio/systemprompt-internal \
  -p 'systemprompt-internal-0.62.0-linux-amd64.tar.gz' -p 'SHA256SUMS*'

# Verify SHA256
grep systemprompt-internal-0.62.0-linux-amd64.tar.gz SHA256SUMS | sha256sum -c -

# Extract
tar -xzf systemprompt-internal-0.62.0-linux-amd64.tar.gz
cd systemprompt-internal-0.62.0-linux-amd64
./bin/systemprompt --version
```

## Verify signature

```bash
cosign verify-blob \
  --certificate-identity-regexp='https://github.com/systempromptio/systemprompt-internal/' \
  --certificate-oidc-issuer='https://token.actions.githubusercontent.com' \
  --signature SHA256SUMS.sig \
  --certificate SHA256SUMS.pem \
  SHA256SUMS
```

## What's in the tarball

| Path | Purpose |
|---|---|
| `bin/systemprompt` | Server and CLI |
| `bin/systemprompt-mcp-*` | The MCP servers (`systemprompt-mcp-agent`, `systemprompt-mcp-odoo`, `systemprompt-mcp-knowledge-bank`) |
| `services/` | YAML configuration tree |
| `extensions/mcp/*/manifest.yaml` | MCP extension manifests |
| `scripts/` | Operator scripts |

Migrations and web templates are compiled into the binary; `web/dist` is
generated at boot by the `publish_pipeline` job.

## Run

You need Postgres 18 and a profile (`.systemprompt/profiles/<name>/profile.yaml`
with its `secrets.json`, including at least one AI provider key). From a
checkout, `just setup-local` writes both and starts a local database. Then:

```bash
systemprompt infra db migrate --profile local
systemprompt infra services start --profile local
```

Docs: https://systemprompt.io/documentation/?utm_source=binary&utm_medium=install_doc
