# Branching: `next` and `main`

Two branches. `next` is where every change lands; `main` only ever moves
through a frozen promotion PR, and a push to `main` *is* a release. Which core
the workspaces build against is the one other thing the branch decides.

| | `next` | `main` |
|---|---|---|
| Purpose | development; the default branch every agent and session works on | releases only; what `release.yml` publishes |
| Core | the published core on crates.io, **or** unreleased core `next` from the sibling `../systemprompt-core` while a core cycle is in flight | **always the published core** |
| Core patch in `Cargo.toml` **and** `tests/Cargo.toml` | dormant (`[workspace.metadata.unreleased-core-patch]`), or active (`[patch.crates-io]` + the `# ACTIVE: core X.Y.Z is unreleased` marker) during a core cycle | dormant |
| Version pins (`systemprompt = "X.Y.Z"`) | lockstep with the repo version; during a core cycle, the sibling tree's workspace version | the published release, equal to the repo version |
| `bridge/CORE_REF` | `vX.Y.Z` while dormant; a 40-char SHA on core `next` (`just core-pin`) while active | `vX.Y.Z`, the tag of the pinned release |
| CI on push | **Gates** (`.github/workflows/gates.yml`) — no images, nothing published | **Release** (`.github/workflows/release.yml`): `bridge-vX.Y.Z`, `vX.Y.Z` and the image — the Gates ran on the push to `next`, which `just release` and `release.yml` both verify; `coverage.yml` measures the merge commit without blocking |
| Deploys | `just deploy` / `just deploy-next` build and ship a working tree (`core-guard` refuses a dirty or unpinned sibling while the patch is active) | `just deploy-release X.Y.Z` ships the published release from a clean worktree of `origin/main` |
| Allowed to break? | yes — a red gate on `next` is information, not an incident | no — every push is a release |

`main` is protected by a ruleset that requires a pull request (and should
require the `Gates passed` check) with no bypass for anyone. Nothing is
committed directly to `main`.

## Lockstep versioning

This repository's version **is** the core version it builds against: the
workspace `version`, `bridge/Cargo.toml`, every core crate pin in both
workspaces, `bridge/CORE_REF` (`vX.Y.Z`), the Helm `appVersion`, the deploy
pins, the git tags `vX.Y.Z` / `bridge-vX.Y.Z` and the image tag `:X.Y.Z`.
`scripts/sync-release-version.sh X.Y.Z` writes all of it and calls
`scripts/sync-core-version.sh X.Y.Z` for the core-pin half;
`scripts/check-release-version.sh` (a lint gate) refuses any drift. A release
of this repo therefore follows a core release.

## One core checkout, one place

`bridge/` depends on `systemprompt-bridge` by **path** — it is not published —
so a core checkout must exist at `../systemprompt-core` on both branches. The
server workspaces, when patched, resolve through the same sibling path. In CI
the composite action `.github/actions/core-checkout` checks core out at
`bridge/CORE_REF` inside the workspace and symlinks the sibling path onto it.
Locally, `just core-checkout` clones it when absent and never moves an
existing checkout: a mismatch with `bridge/CORE_REF` is fatal
(`MISMATCH=fail`, packaging) or a warning (`MISMATCH=warn`, `bridge-build`
and `bridge-preview`).

## Three lockfiles, one core

`bridge/Cargo.toml` depends on `systemprompt-bridge` by path
(`../../systemprompt-core/bin/bridge`), and `tests/` is a separate workspace
with its own `Cargo.lock`, so there are three lockfiles and `cargo update -w`
in the root refreshes only one of them. All three must resolve every
`systemprompt-*` crate at **the same version** — a path copy and a registry
copy may coexist, but not two versions, or a test or bridge build carries a
second copy of a shared crate and fails with a type-mismatch error that names
neither lockfile. `scripts/check-core-crate-versions.sh` (`preflight-static`,
the `static` tier) enforces it; `just core-bump` refreshes all three.

## The silent-drop trap

A `[patch]` whose version does not satisfy the pin is **dropped without an
error**: cargo resolves the published crate instead, the build passes, and it
has proved nothing about the core you meant to test. For a 0.x crate
`version = "0.61.0"` means `>=0.61.0, <0.62.0`, so a patch pointing at a
0.62.0 tree is ignored by a 0.61.0 pin.

Defences, in the order they catch it:

1. `scripts/check-release-version.sh` (a lint gate) — with the patch active,
   the sibling core's workspace version must equal the pins.
2. `scripts/sync-core-version.sh <core-version> --check` — every core pin in
   both workspaces must agree (a residual sweep fails on any pin it does not
   move).
3. `scripts/check-core-ref.sh` (a lint gate) — with the patch dormant,
   `bridge/CORE_REF` must be `v<pin>`.
4. The build log. With the patch active it must name `../systemprompt-core/...`
   paths for the `systemprompt-*` crates; a bare `v0.62.0` with no path is a
   dropped patch. `release.yml` additionally refuses a `main` whose
   `Cargo.lock` does not resolve `systemprompt` from the registry.

## Working on `next`

```bash
just verify                      # what gates.yml runs: static, lint, tests
git push origin next             # gates.yml runs; nothing is published
just deploy                      # build this tree and ship it (core-guard first)
```

During a core cycle (patch active), also `just core-pin` so
`bridge/CORE_REF` names the core commit you built against — push core first,
because CI checks that ref out of GitHub. Core changes are **write-only from
here**: commit them on core's `next` and push; core's own CI is their
validation surface.

## Landing `next` on `main` (once core X.Y.Z is on crates.io)

```bash
# Make both patch blocks dormant again, then:
just core-bump X.Y.Z             # every pin + CORE_REF, all three lockfiles, migrate/build/clippy
just schema-baseline             # record tests/fixtures/schema/release-baseline-X.Y.Z.sql
# CHANGELOG: move ## Unreleased under ## X.Y.Z
git commit -am "release: X.Y.Z against published core"
git push origin next             # the full Gates matrix runs here
just release X.Y.Z               # verify the push proof, open the frozen PR
just release X.Y.Z               # again, once the PR's run finishes: verify and merge
```

`just release` refuses when the tree is dirty, when run off `next` (a clean
detached worktree at `origin/next` also qualifies), when a patch block is
active, when a pin or `CORE_REF` disagrees, when the schema ladder is missing
the version's rung, when HEAD is not `origin/next`, when `main` is not an
ancestor, or when the latest next-push Gates run on that SHA is missing,
pending or red. The merge triggers `release.yml`; what it produces is in
[RELEASING.md](RELEASING.md). Once it is green, `just deploy-release X.Y.Z`
ships that release to production from a clean worktree of `origin/main`.
