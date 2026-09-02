# Releasing

The process for shipping a new gateway release when a new `systemprompt` core
version lands on crates.io.

**What is automatic and what is not.** Nothing gates a push to `next`. The
release pull request onto `main` (`just gate` → `just promote`) runs CI and
Quality. Merging it is the release act: `.github/workflows/release.yml` fires
on the push to `main`, re-runs CI and Quality on the merge commit, and — only
if both pass — publishes the desktop bridge for macOS, Windows and Linux as
GitHub Release `bridge-v<version>` and the container image
`ghcr.io/systempromptio/systemprompt-internal:<version>` (also `:latest`).
Deploying the instance (`just deploy`) is still a hand step.

## Versioning policy

The fork tracks core in lockstep: core `X.Y.Z` on crates.io → workspace
`version = X.Y.Z` → git tag `vX.Y.Z` → Helm `appVersion: X.Y.Z` (the chart's
own `version:` gets a minor bump per release, handled by the sync script)
→ bridge `bridge/Cargo.toml` `version = X.Y.Z` and `bridge/CORE_REF` =
`vX.Y.Z` → GitHub Release `bridge-vX.Y.Z` → image tag `:X.Y.Z`. One number
everywhere; `scripts/sync-release-version.sh` writes it and
`scripts/check-release-version.sh` (a lint gate) refuses drift. The release
workflow will not publish a `main` whose pins disagree, or whose
`bridge/CORE_REF` names a core commit whose own version is not that number
(`CORE_REF` is `vX.Y.Z` after `just core-bump`; on `next` it is a SHA on core
`next`). Every release job builds against that core checkout, exactly as CI
does — with the patch active it is the sibling, without it crates.io.

## Step A0 — adopting an *unpublished* core (the patched path)

Most core versions are adopted here before they are on crates.io: the sibling
`../systemprompt-core` checkout is bumped, this repo is patched onto it via
`[patch.crates-io]`, and the two are proven together *before* core publishes.
`just core-bump` deliberately refuses to run in this state — it is the
published-crates path — so this step is by hand and nothing reminds you.

**Bump the pins first, and bump all of them.** A version requirement that no
longer matches the patched crate does not error: cargo silently drops the
patch and resolves the old version from crates.io, so the build "works" while
proving nothing about the new core. The pins live in **two** manifests, because
`tests/` is a separate workspace with its own copy:

```bash
grep -rnE '^systemprompt[a-z-]* = (\{ version = )?"' --include=Cargo.toml . | grep -v target
sed -i 's/OLD/NEW/g' Cargo.toml tests/Cargo.toml     # or let the script do it
scripts/sync-release-version.sh NEW --check          # core-pin lines must be silent
```

`sync-release-version.sh` covers every core pin in both manifests plus a
residual sweep that fails on any core pin it does not itself move — so a pin
added to a new crate cannot sit stale. Its remaining `DRIFT:` lines on this
path are the *product* version (workspace version, Chart.yaml, deploy files);
those belong to Step A, not here. Do **not** bump them for a core that has not
shipped.

Then prove it, in this order — each step catches a class the previous one
cannot:

```bash
just build                                    # patch resolved? log must read the new version
just clippy
grep -n 'Breaking' ../systemprompt-core/CHANGELOG.md   # then grep this repo for each item
./target/debug/systemprompt infra db migrate  # new core migrations, against the local DB
./target/debug/systemprompt --version         # must print the new core version
just start && curl -s localhost:8080/health   # must reach {"status":"healthy"}, not "starting"
```

Confirm the build log names `systemprompt-* vNEW (/var/www/html/systemprompt-core/...)`.
A build that compiles registry crates instead is a dropped patch, not a pass.

Three things no gate catches on this path:

- **A tightened identifier validator is a runtime panic, not a compile error.**
  Core's `define_id!(…, validated, …)` types panic in `new()` on a value they
  used to accept, so a construction site that stops being legal still compiles
  and still passes clippy — it fails only when that code path executes. 0.29.0
  did exactly this to `ContextId` (now UUID-v4 only), and
  `hooks_track::build_request_context` had been passing `ContextId::new("")`,
  which would have panicked on every `/hooks/track` AI summary. Sweep for it
  whenever the core diff touches `crates/shared/identifiers`:
  `grep -rn '::new("' --include='*.rs' extensions/ src/` — and prefer
  `try_new` or `generate()` over a literal at any site that cannot prove the
  value's shape.

- **Migrations run silently and are not reversible.** Run them and then check
  the tables the core changelog describes actually exist, rather than trusting
  the success line.
- **A new core job is inert until this repo schedules it.** Core discovers jobs
  by inventory; whether one *runs* comes from `services/scheduler/config.yaml`.
  Boot warns `job is available in this build but has no scheduler.jobs entry`
  once per job and then carries on. Decide per job — scheduling it and
  deliberately leaving it off are both fine, silently missing it is not.

Only once core is published on crates.io do you comment the two
`[patch.crates-io]` blocks (root and `tests/`, in lockstep) and continue with
Step A.

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
(`sync-release-version.sh --check`), and prints the `vX.Y.Z` and
`bridge-vX.Y.Z` releases the merge published (Step C). Nothing is tagged by
hand any more — both tags are created by the workflow at the merge commit.

## Step C — the merge publishes the artifacts

When the release PR merges, `release.yml` on `main`:

1. `version` — reads `bridge/Cargo.toml`, runs the sync check, checks out
   core at `CORE_REF` and asserts its version is `X.Y.Z`. If `bridge-vX.Y.Z`
   already exists (a merge with no bump) every later job is skipped with a
   notice.
2. `ci` + `quality` — the full workflows, called on the merge commit.
3. `checks` → `build` → `release` — bridge fmt/clippy, the four platform
   builds (macOS signed + notarized), cosign-signed assets, GitHub Release
   `bridge-vX.Y.Z` at the merge commit.
4. `gateway` → `release-gateway` — `cargo build --release --workspace` for
   `linux-amd64`, `linux-arm64`, `darwin-arm64`; tarballs with `bin/`
   (gateway + MCP servers), `services/`, extension manifests, `scripts/`;
   cosign-signed `SHA256SUMS`; GitHub Release `vX.Y.Z` at the merge commit.
5. `publish-image` — `docker.yml`: multi-arch image, `:X.Y.Z`, `:X.Y`, `:X`,
   `:latest`, `:sha-…`, cosign-signed, smoke-run.

Re-publish a release without re-merging with
`gh workflow run release.yml -f bridge_tag=bridge-vX.Y.Z` (the tag must equal
the manifest version). Deploy the instance with `just deploy`; the admin
Bridge Setup page links `releases/download/bridge-vX.Y.Z/…` by the running
binary's version, so the links are right the moment the deploy lands.

The desktop bridge's self-updater reads `gateway.bridge_releases` from the
production profile (repo, `tag_prefix: bridge-v`, the four `assets:`, no
`pinned_version`): `main` is the only publisher, so "newest release" is the
build shipped with the deployed core. `SYSTEMPROMPT_BRIDGE_RELEASES_TOKEN`
(fine-grained PAT, contents:read) keeps the gateway off GitHub's anonymous
rate limit.

### Retention

`ghcr-prune.yml` runs weekly and after every successful release: it keeps the
3 newest `X.Y.Z` images (alias tags follow), drops `sha-*` tags and untagged
manifests older than 4 weeks, and keeps the 3 newest `v*` and the 3 newest
`bridge-v*` releases — one of each per core release — deleting older ones
with their tags and any `bridge-v*` tag left without a release
(`scripts/prune-releases.sh`; drafts and prereleases are never touched). It
needs `GHCR_PRUNE_TOKEN` — a classic PAT with `read:packages` +
`delete:packages` — and fails loudly without it.

## Rollback

1. Redeploy the previous good build (`just deploy` from the previous tag).
2. Mark any GitHub Release as pre-release or delete it.
3. Never reuse a tag — fix forward and cut the next patch version.
4. Chart: publish the previous chart again or a new patch chart pinning the
   good image via `image.tag`.

## Post-release checklist

- [ ] `just verify` green on the tagged commit
- [ ] `gh run list --workflow=release.yml --limit 1` green: release
      `bridge-vX.Y.Z` and image `:X.Y.Z` exist
- [ ] the deployed instance serves the new version (`just server-status`)
      and `/admin/bridge/setup` links `bridge-vX.Y.Z`
- [ ] Helm chart packaged only if that release is actually being distributed
- [ ] update docs-internal/STATE.md release row
