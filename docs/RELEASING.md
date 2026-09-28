# Releasing

How a new `systemprompt` core version becomes a release of this repo: the
desktop bridge for macOS, Windows and Linux (GitHub Release
`bridge-vX.Y.Z`), the gateway server tarballs (GitHub Release `vX.Y.Z`) and
the container image `ghcr.io/systempromptio/systemprompt-internal:X.Y.Z` —
all produced by CI when a frozen promotion PR merges to `main`.

**What is automatic and what is not.** `.github/workflows/gates.yml` runs the
full gate on every push to `next` and on ordinary PRs.
`just release X.Y.Z` promotes the exact green `next` commit through a frozen PR;
merging it is the release act, and `.github/workflows/release.yml` publishes
everything from the merge commit without re-running the gates.
`.github/workflows/coverage.yml` measures coverage on `main` and nightly; it is
not a release check. Deploying production (`just deploy-release X.Y.Z`) is a
hand step. The branch contract that makes this safe is in
[BRANCHING.md](BRANCHING.md); read it first.

## Versioning policy

The fork tracks core in **lockstep**: core `X.Y.Z` on crates.io → workspace
`version = X.Y.Z` → git tags `vX.Y.Z` and `bridge-vX.Y.Z` → Helm
`appVersion: X.Y.Z` (the chart's own `version:` gets a minor bump per release,
handled by the sync script) → `bridge/Cargo.toml` `version = X.Y.Z` and
`bridge/CORE_REF` = `vX.Y.Z` → image `:X.Y.Z`. One number everywhere;
`scripts/sync-release-version.sh X.Y.Z` writes it (calling
`scripts/sync-core-version.sh X.Y.Z` for every core pin and `CORE_REF`), and
`scripts/check-release-version.sh` (a lint gate) refuses drift. The release
workflow will not publish a `main` whose pins disagree, whose patch is active,
or whose `bridge/CORE_REF` names a core commit whose own version is not that
number.

Because the version is core's, a merge that does not bump it has nothing new
to publish: `release.yml`'s `version` job sees `bridge-vX.Y.Z` already exists
and every later job is skipped with a notice.

## Step A0 — adopting an *unpublished* core (the patched path)

Most core versions are adopted here before they are on crates.io: the sibling
`../systemprompt-core` checkout is bumped, this repo is patched onto it, and
the two are proven together *before* core publishes. `just core-bump`
deliberately refuses to run in this state — it is the published-crates path —
so this step is by hand and nothing reminds you.

**Activate the patch in both manifests.** Rename the dormant
`[workspace.metadata.unreleased-core-patch]` table to `[patch.crates-io]` in
`Cargo.toml` and `tests/Cargo.toml` (`[patch]` applies per workspace) and add
the `# ACTIVE: core X.Y.Z is unreleased` marker above it — the pre-commit hook
refuses an active patch without it.

**Bump the pins first, and bump all of them.** A version requirement that no
longer matches the patched crate does not error: cargo silently drops the
patch and resolves the old version from crates.io, so the build "works" while
proving nothing about the new core.

```bash
scripts/sync-core-version.sh NEW              # every core pin, both workspaces
bash scripts/check-release-version.sh         # sibling version == pins?
```

Do **not** move this repo's own version (workspace, bridge, Helm, deploy
files) for a core that has not shipped; that belongs to Step A.

Then prove it, in this order — each step catches a class the previous one
cannot:

```bash
just build                                    # patch resolved? log must read the new version
just clippy
grep -n 'Breaking' ../systemprompt-core/CHANGELOG.md   # then grep this repo for each item
./target/debug/systemprompt infra db migrate --profile local
./target/debug/systemprompt --version         # must print the new core version
just start && curl -s localhost:8080/health   # must reach {"status":"healthy"}, not "starting"
```

Confirm the build log names `systemprompt-* vNEW (.../systemprompt-core/...)`.
A build that compiles registry crates instead is a dropped patch, not a pass.
Then `just core-pin` so `bridge/CORE_REF` names the core commit you built
against (push core first — CI checks that ref out of GitHub), and push `next`:
`gates.yml` runs the same tiers as `just verify` against that core.

Three things no gate catches on this path:

- **A tightened identifier validator is a runtime panic, not a compile error.**
  Core's `define_id!(…, validated, …)` types panic in `new()` on a value they
  used to accept, so a construction site that stops being legal still compiles
  and still passes clippy — it fails only when that code path executes. 0.29.0
  did exactly this to `ContextId` (now UUID-v4 only). Sweep for it whenever the
  core diff touches `crates/shared/identifiers`:
  `grep -rn '::new("' --include='*.rs' extensions/ src/` — and prefer
  `try_new` or `generate()` over a literal at any site that cannot prove the
  value's shape.
- **Migrations run silently and are not reversible.** Run them and then check
  the tables the core changelog describes actually exist, rather than trusting
  the success line. What proves an upgrade from a *deployed* database is the
  schema ladder: every rung is restored and migrated forward by the current
  installer (`tests/integration/schema-upgrade`), and `release.yml`'s
  `upgrade-boot` boots the published image over every rung with rows in the
  hot tables.
- **A new core job is inert until this repo schedules it.** Core discovers jobs
  by inventory; whether one *runs* comes from `services/scheduler/config.yaml`.
  Boot warns `job is available in this build but has no scheduler.jobs entry`
  once per job. Decide per job — scheduling it and deliberately leaving it off
  are both fine, silently missing it is not.

### `bridge/CORE_REF` gates the whole remote proof

Every Gates tier materialises the sibling core checkout at the ref in
`bridge/CORE_REF` (`.github/actions/core-checkout`). It is therefore not only
the bridge's pin: **it decides which core CI compiles against** while the
patch is active.

- During a core cycle it is a 40-char SHA on core's `next` (`just core-pin`).
  Advance it whenever core `next` moves, or the gates run against an older
  core.
- With the patch dormant it is `vX.Y.Z`; `scripts/check-core-ref.sh` (a lint
  gate) enforces that it matches the pins.

`bridge/Cargo.toml` is a third manifest but not a third patch block: it takes
core by a bare path dep (`systemprompt-bridge = { path = "../../systemprompt-core/bin/bridge" }`),
so it is sibling-coupled on every branch. There are three lockfiles —
`Cargo.lock`, `tests/Cargo.lock`, `bridge/Cargo.lock` — and `cargo update -w`
re-resolves the root workspace only; re-resolve each explicitly after toggling
the patch.

## Step A — adopt the published core (on `next`)

Make both patch blocks dormant again, then:

```bash
just core-bump X.Y.Z
```

This refuses to run with an active `[patch.crates-io]`, then runs
`scripts/sync-release-version.sh X.Y.Z` (the workspace and bridge versions,
every core pin and `bridge/CORE_REF` via `sync-core-version.sh`, Chart.yaml
appVersion + chart version + artifacthub annotation/changelog, and the
exact-pin deploy files: CasaOS compose, DigitalOcean compose + Packer
default), re-resolves all three lockfiles, runs
`infra db migrate --profile local`, `just build` and `just clippy`.

**`core-bump` is local-only.** The migrate step names `--profile local` and is
not `|| true`-swallowed: the 0.51.0 bump ran a bare `infra db migrate` after
`just deploy-check` had flipped the CLI's active session to production, and
the migration was pointed at the live database. Core's CLI now refuses
`infra db migrate` and `infra jobs run` on an implicitly selected cloud
profile, and an explicit `--profile` never rewrites the saved session.

Then:

1. **Record the schema rung.** `just schema-baseline` writes
   `tests/fixtures/schema/release-baseline-X.Y.Z.sql` from a fresh local
   install. The ladder is append-only, one rung per release from the floor
   (0.61.0); `scripts/check-schema-baseline.sh` fails without the rung for the
   workspace version. A rung for an already-published release is recorded from
   its tarball with `just schema-baseline X.Y.Z` (Linux hosts).
2. `just prepare` if query or schema inputs changed.
3. Move the CHANGELOG's `## Unreleased` entries under `## X.Y.Z`.
4. `just verify` (optional; the push runs it), commit, `git push origin next`.

**Lockfile agreement.** `scripts/check-core-crate-versions.sh`
(`preflight-static` and the `static` tier) fails when the three lockfiles
resolve any `systemprompt*` crate at more than one version — a stale lockfile
compiles against a different core than the one being released and surfaces as
an unrelated compile error deep in a test or bridge build (0.51.0).

**Docker.** `cloud deploy` shells out to `docker build`. `deploy` and
`deploy-next` run `_docker-preflight` first: when `/usr/bin/docker` exists,
`/usr/bin` is pinned to the front of `PATH`, and if `docker` still resolves
elsewhere a warning names the path. A wrapper shim ahead of the real binary
(0.51.0) fails the image build with an error that mentions neither.

## Step B — promote the proven `next` commit

```bash
just release X.Y.Z
```

Run from a clean `next` (or a detached worktree whose HEAD is `origin/next`).
`scripts/release.sh` checks the patch is dormant, every pin agrees
(`sync-release-version.sh X.Y.Z --check`, `check-release-version.sh`,
`check-core-ref.sh`, `check-schema-baseline.sh`), HEAD is `origin/next`,
`main` is an ancestor, and the latest `gates.yml` run for a push to `next` on
that exact SHA completed green with its `Gates passed` aggregate
(`scripts/check-gates-green.sh`). It then pushes
`promote/X.Y.Z/<main-sha>/<next-sha>` and opens the PR.

The PR's Gates run skips the matrix and runs only `Verify frozen promotion`:
the push proof, unchanged head and base, ancestry, and a GitHub merge tree
identical to the candidate. Record the PR and run IDs, and once that run
finishes, repeat `just release X.Y.Z`: it re-verifies both proofs
(`scripts/check-promotion-green.sh`) and the proposed merge tree, merges with
`--match-head-commit`, and checks the merged parents and tree. It never pushes
to `main`. Missing, pending, cancelled or red proof all block promotion.

The self-tests in `tests/scripts/` (run by the `static` tier) exercise these
scripts against mocked `git`/`gh` state, including every refusal.

## Step C — the merge publishes the artifacts

On the push to `main`, `release.yml`:

1. `version` — `scripts/check-release-merge.sh` proves the commit is the
   two-parent merge of a frozen promotion PR whose tree is the candidate's,
   at the version the ref names, with both proofs green; it asserts the patch
   is dormant, `Cargo.lock` resolves `systemprompt` from crates.io and
   `check-core-ref.sh` passes; reads `bridge/Cargo.toml`, runs the sync and
   lockstep checks, checks out core at `CORE_REF` and asserts its version is
   `X.Y.Z`. If `bridge-vX.Y.Z` already exists every later job is skipped.
2. `checks` → `build` → `release` — bridge fmt/clippy, the four platform
   builds (macOS universal, signed + notarized), cosign-signed assets, GitHub
   Release `bridge-vX.Y.Z` at the merge commit, then a probe that every
   published download link resolves (versioned and `releases/latest`).
3. `publish-image` — `docker.yml`: multi-arch image pushed by digest, merged
   and cosign-signed as **`:sha-<short>` only**.
4. `smoke` and `upgrade-boot` prove that digest: `--version` reports `X.Y.Z`,
   all three MCP binaries are present, both arches, signature verified; and
   the image's own entrypoint boots over every schema rung seeded with 2000
   rows per hot table (`seed_hot_tables.sql`) — `/health` healthy or `/readyz`
   — without losing an `ai_requests` row.
5. `promote-tags` — only then point `:X.Y.Z` (create-once), `:X.Y`, `:X` and
   `:latest` at the proven digest. A failed run leaves nothing but its own
   `:sha-<short>`.
6. `gateway` → `release-gateway` — `cargo build --release --workspace` for
   `linux-amd64`, `linux-arm64`, `darwin-arm64`; tarballs with `bin/`
   (gateway + MCP servers), `services/`, extension manifests, `scripts/`;
   cosign-signed `SHA256SUMS`; GitHub Release `vX.Y.Z` at the merge commit,
   published after `promote-tags` because its notes name the image.

Re-publish a release without re-merging with
`gh workflow run release.yml -f bridge_tag=bridge-vX.Y.Z` (the tag must equal
the manifest version). Nothing is tagged by hand.

The admin Bridge Setup page links `releases/download/bridge-vX.Y.Z/…` by the
running binary's version, so the links are right the moment the deploy lands.
The desktop bridge's self-updater reads `gateway.bridge_releases` from the
production profile (repo, `tag_prefix: bridge-v`, the four `assets:`, no
`pinned_version`): `main` is the only publisher, so "newest release" is the
build shipped with the deployed core. Two release series share this repo and
GitHub hands `releases/latest` to the newest release, so the bridge release
claims `latest` and the gateway release declines it.
`SYSTEMPROMPT_BRIDGE_RELEASES_TOKEN` (fine-grained PAT, contents:read) keeps
the gateway off GitHub's anonymous rate limit.

## Step D — deploy production (Fly)

```bash
just deploy-release X.Y.Z
```

`scripts/deploy-release.sh` works from a clean detached worktree of
`origin/main` (the `vX.Y.Z` tag must *be* `origin/main`, and `bridge-vX.Y.Z`
and the `:X.Y.Z` image must exist), installs the release's own server
binaries with `just fetch-release` (verified against the release
`SHA256SUMS`), renders `web/dist` there, runs `cloud doctor`, deploys, then
requires the Fly machine's image digest **and** update timestamp to move,
waits for `/health` to report healthy on core `X.Y.Z`, and watches it for five
minutes. `DEPLOY_DRY_RUN=1` stops before `cloud deploy`. Linux x86_64 hosts
only (the tarball's binaries are baked into the linux/amd64 image and run
there). The worktree is kept for inspection on failure.

`just deploy` remains the preview path: it builds and ships whatever this
working tree holds (`core-guard` first). `just deploy-next` does the same from
a dedicated worktree of `origin/next`.

## Retention

`ghcr-prune.yml` runs weekly and after every successful release: it keeps the
3 newest `X.Y.Z` images (alias tags follow), drops `sha-*` tags and untagged
manifests older than 4 weeks, and keeps the 3 newest `v*` and the 3 newest
`bridge-v*` releases — one of each per core release — deleting older ones
with their tags and any `bridge-v*` tag left without a release
(`scripts/prune-releases.sh`; drafts and prereleases are never touched). It
needs `GHCR_PRUNE_TOKEN` — a classic PAT with `read:packages` +
`delete:packages` — and fails loudly without it.

## Rollback

1. Redeploy the previous good release: `just deploy-release <previous>` (or
   `just deploy` from the previous tag's worktree).
2. Mark any GitHub Release as pre-release or delete it.
3. Never reuse or move a released tag — fix forward and cut the next version.
4. Core migrations are forward-only; roll back only to a version whose
   migrations match the database.
5. Chart: publish the previous chart again or a new patch chart pinning the
   good image via `image.tag`.

## Post-release checklist

- [ ] `gh run list --workflow=release.yml --limit 1` green: `bridge-vX.Y.Z`,
      `vX.Y.Z` and image `:X.Y.Z` exist, `promote-tags` succeeded
- [ ] `just deploy-release X.Y.Z` finished its five-minute watch
- [ ] the deployed instance serves the new version (`just server-status`)
      and `/admin/bridge/setup` links `bridge-vX.Y.Z`
- [ ] Helm chart packaged only if that release is actually being distributed
- [ ] update docs-internal/STATE.md release row
