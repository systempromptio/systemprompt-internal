# Releasing

The process for shipping a new gateway release when a new `systemprompt` core
version lands on crates.io.

**This repository runs no hosted CI.** There are no GitHub Actions workflows:
it is a private repo and paying for runner minutes buys nothing a local
machine cannot do. Every gate is a local command, and every artifact is built
by hand. Nothing happens automatically when you push a branch or a tag — if
you did not run it, it did not run.

## Versioning policy

The fork tracks core in lockstep: core `X.Y.Z` on crates.io → workspace
`version = X.Y.Z` → git tag `vX.Y.Z` → Helm `appVersion: X.Y.Z` (the chart's
own `version:` gets a minor bump per release, handled by the sync script).

## Step A — bump and validate locally

```bash
just core-bump X.Y.Z
```

This refuses to run with an active `[patch.crates-io]` override, then runs
`scripts/sync-release-version.sh X.Y.Z` (bumps the workspace version, the
`systemprompt` + `systemprompt-security` pins, Chart.yaml appVersion +
chart version + artifacthub annotation/changelog, and the exact-pin deploy
files: CasaOS compose, DigitalOcean compose + Packer default), runs
`cargo update -w`, migrations against the local DB, `just build`, and
`just clippy`.

Then exercise anything the core changelog touches, review the diff, and run
the full gate:

```bash
just verify
```

`verify` is the whole check in one command — `cargo fmt --check`, the offline
sqlx cache, the 19 source gates, clippy at `-D warnings`, and the unit,
integration, and admin-contract test suites. It is what a CI pipeline would
have run. Commit to main and push only once it is green.

## Step B — tag

```bash
just release X.Y.Z
```

Checks the tree is clean, HEAD == origin/main, every pin matches
(`sync-release-version.sh --check`), and `just verify` passes, then pushes the
`vX.Y.Z` tag. The tag is a marker: nothing consumes it.

## Step C — build and publish artifacts

By hand, from a clean checkout of the tag:

```bash
just build-all                    # release binary, MCP servers, web assets
just docker-build X.Y.Z           # container image, if one is being shipped
```

Push the image, attach binaries to a GitHub release, and package the Helm
chart only if that release is actually being distributed. Most releases of
this fork are deployed straight to the instance with `just deploy` and need
none of it.

## Rollback

1. Redeploy the previous good build (`just deploy` from the previous tag).
2. Mark any GitHub Release as pre-release or delete it.
3. Never reuse a tag — fix forward and cut the next patch version.
4. Chart: publish the previous chart again or a new patch chart pinning the
   good image via `image.tag`.

## Post-release checklist

- [ ] `just verify` green on the tagged commit
- [ ] the deployed instance serves the new version (`just server-status`)
- [ ] any published image or chart actually pushed — nothing does it for you
- [ ] update docs-internal/STATE.md release row
