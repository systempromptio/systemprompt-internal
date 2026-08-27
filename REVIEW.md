# Review guide — 2026-08-27

Five agent sessions worked this repository and `../systemprompt-core` on the
same day. Four of them exited before their work was committed; this document,
and the commits it describes, were produced by the fifth consolidating what they
left behind.

It is organised by **what could be wrong**, not by commit, because the commit
order is an artifact of who finished when and tells a reviewer very little.

Everything below is on `next`. Nothing has gone to `main` — no gate, no promote,
no release PR, and deliberately no `bridge-v*` tag.

---

## 1. What landed

### Session registry — `6dad6aa5`

`plugin_session_summaries` recorded what a session did and nothing about where or
when it was doing it, so nothing could address a session and no report could
attribute cost to a repository. Adds `cwd`, `workspace`, `git_branch`, `handle`,
`last_event_at`, `current_activity`, `live_cost_microdollars` and `context_pct`,
all from signals the hook plane already sent and discarded. `handle` is the
address — derived from the workspace, unique per user across *live* sessions
only.

Also implements `/hooks/statusline`, which previously authenticated and returned
204 with its payload unbound, throwing away the model id, running cost and
context-window usage; its authenticator discarded its JWT claims too, so rows had
no user attribution. Both fixed. Carries an APM correctness fix with a
consequence — see *Known-weak spots*.

### Team comms — `cce76183`, `398554be`, `33256821`, wired in `071efd34`

Five tools on port 5060 for addressed messages between people and their agent
sessions. The address form decides whether a message may interrupt: `@user` and
`#channel` are stored and raise an unread count but enter no running session;
only `@user/session-handle` reaches a session, and only the one it names.
Addressing a session that has gone idle degrades to the inbox rather than
failing, so a sender never has to check whether a peer is online first.

Ships the team-inbox Cowork dashboard. It **peeks** rather than reads, so opening
the board does not hide messages from the agent they were addressed to.

This landed inert on the day: the crate was not a workspace member and the config
was not included, because the files that would have wired it in also carried the
then-uncommitted email server. `071efd34` is where it actually goes live.

### Outbound email — `761f7109`, demo script in `bf3ce3c4`

`email_send`, the only tool here that reaches outside the company. Two MRTR
rounds (SEP-2322): the first elicits confirmation from the drafter, the second
runs `require_approval`, which holds the call for a *different* human. No
draft/send tool pair and no bypass token — a client that does not implement MRTR
never gets past round one and never sends.

### Artifact renderer branding — `e5f7bf5e`

Core ships deliberately unbranded neutral-slate tokens and expects a deployment
to re-declare the `--mcpui-*` properties it cares about. This one declared none,
so every artifact rendered into Cowork came back cool-blue and square-cornered
against a warm-orange product. Adds the theme, plus `just artifact-gallery`,
which renders every artifact type and rasterizes each in light / dark / narrow.

### Governance approvals — `34434f4e`, `4321c08d`, core `bee179e99` / `f4e907d59`

`require_approval` is the fifth stage and the only one returning a third verdict,
`Decision::Pending`: neither allowed nor denied, but held for a named human.

**Read the next paragraph before trusting that sentence anywhere outside this
repo.** In `systemprompt-core`, that stage has no production caller: the MCP
plane consumes `AuthzDecision`, which has no `Pending` variant, and both planes
that see `Decision::Pending` convert it to a deny. Migration 014 and
`approval_requests` shipped ahead of the enforcement point.

This installation is unaffected, because **the enforcement point is here, not in
core.** `extensions/mcp/shared/src/approval/` is internal's own gate, called
directly by the odoo and email servers before the tool body runs
(`odoo/src/server/mod.rs:44`, `email/src/server/tool.rs:35`). It evaluates only
the `require_approval` policy from `GovernanceEngine::global()` — deliberately
not the whole chain, which would charge the shared rate limiter twice — and
writes the approval row itself. That is why
`a_non_admin_send_is_held_for_a_second_human_and_only_flies_once_approved`
passes: the hold is real here and proven end to end.

The distinction matters if this code is ever read as a description of core's
behaviour, or if a future change tries to route internal's holds through core's
stage on the assumption it already works.

---

## 2. Review order

Dependency order, not chronological. Each unit names the one question to try to
answer.

**1. Schema — `6dad6aa5`, `extensions/web/schema/migrations/030_session_registry.sql`**
Everything else addresses a session, and this is what makes a session
addressable. *Does the partial unique index actually release a handle when a
session ends?*

**2. Session registry repositories and the statusline handler — `6dad6aa5`**
The writers that fill the columns. *Is every new column actually written by
something?* (One is not. See *Known-weak spots*.)

**3. Comms server — `cce76183`, particularly `store/reads.rs`**
The inbox predicate is the isolation boundary for the whole feature. *Can any
arm of it be made to match a sibling session's message?*

**4. Wiring — `071efd34`**
Where comms and email share a diff, and where both become reachable. *Are the
two RBAC grants defensible, given each is `[user]`?*

**5. Email server — `761f7109`**
Depends on approvals being real. *Is there any path through this tool that
reaches the relay without two humans?*

**6. Bridge and gateway — core `abbf25b3c..f4e907d59`, comms leg in `59fc30e59`**
The push leg. *Does anything here ship to a machine?* (It should not — no
`min_bridge_version` bump, no `bridge-v*` tag, confirmed.)

Two bugs were found and fixed inside this during core's quality pass, both worth
re-reading rather than assuming: `comms_drain` read the inbox and then unlinked
it, destroying any append that landed between the two calls — the exact loss its
own comment claimed the design avoided; it now renames first, with the pid in the
taken name. And the hook command interpolated the exe path unquoted, so any
install under a path containing a space produced a command that split at it.

---

## 3. The load-bearing claims

Each claim below is something the day's work asserts. Each has one check that
proves it. If you only have time to verify four things, verify these.

### A session never sees a sibling session's messages

This is the whole isolation boundary of the comms server. Two sessions belonging
to the *same person* must not read each other's traffic, or `@user/handle`
addressing means nothing.

The boundary is a single SQL predicate — the inbox read matches three arms: the
message names this session, the message names this user and names no session, or
the message is in a channel this session subscribes to. A sibling session's
addressed message matches none of them.

**Check:** `tests/e2e/src/comms.rs` —
`a_message_addressed_to_one_session_never_reaches_its_sibling` registers two live
sessions for one user, sends to one, and asserts the other's inbox is empty.
`an_unaddressed_user_sees_nothing` is the negative control.

Read the predicate in `extensions/mcp/comms/src/store/reads.rs` alongside the
test. The question to answer: *is there a fourth arm, or an OR that widens it?*

### Handles are released when a session ends

`handle` is the address. If an ended session kept its handle forever, a person
who has run a hundred sessions in one repo could never be addressed again.

**Check:** the partial unique index in
`extensions/web/schema/migrations/030_session_registry.sql:43`:

```sql
CREATE UNIQUE INDEX IF NOT EXISTS idx_session_summary_handle
    ON plugin_session_summaries (user_id, handle)
    WHERE handle IS NOT NULL AND ended_at IS NULL;
```

The `WHERE ended_at IS NULL` is the entire mechanism. Uniqueness is scoped to
live sessions, so ending a session releases its address without a delete or a
rename. The question to answer: *does anything set `ended_at` back to NULL, or
reuse a row across sessions?* If so the index stops meaning this.

### The governance chain is not double-evaluated in-server

Agent tool calls are already governed at the hook plane before they reach an MCP
server. `GovernanceEngine` is a process singleton, so a second in-server
evaluation would not just be redundant — it would double-count the rate limiter,
and a caller would be throttled at half the configured rate with nothing in the
audit log explaining why.

**Check:** the comms server deliberately does **not** call the policy chain. Grep
`extensions/mcp/comms/` for `GovernanceEngine` / `evaluate` and confirm the
absence is real rather than an oversight.

The email server is the deliberate exception and is worth contrasting: it *is*
held, but by the `require_approval` stage configured in
`services/governance/config.yaml`, evaluated once, at the hook plane, on the way
in — not a second chain inside the binary.

### Artifacts execute as the viewer, so the board cannot leak across users

The team-inbox Cowork dashboard renders one person's messages. If it fetched over
HTTP with the server's credentials, a markup bug would show a viewer someone
else's inbox.

It does not. It reads through `window.cowork.callMcpTool`, executing as the
signed-in viewer — there is no HTTP path into an artifact at all. The isolation
is therefore structural, not a property of the template being correct.

It also *peeks* rather than reads, so opening the board does not clear unread
state out from under the agent the messages were addressed to.

**Check:** grep the dashboard for `fetch(` and for any bearer token. Finding
either would break the claim. The question to answer: *does every data path in
the artifact go through `callMcpTool`?*

---

## 4. Known-weak spots

Stated plainly, because a reviewer will find them anyway and should find them
here first.

**`git_branch` has no writer.** The column is declared in both the schema and
migration 030, read in six places (`live_sessions.rs`, the sessions SSR handler,
the comms store and its renderer), and populated by nothing. Client hooks are
generated by the bridge from a fixed template rather than shipped as scripts, so
filling it needs a bridge change that has not been made. It renders as `—`
today. This is a known hole, not an oversight — but it does mean the
"per-repository attribution" story is `workspace`-only for now.

**Nothing has run against a live Cowork session.** The push leg — bridge comms
consumer, the `bridge_stream` gateway route, the AG-UI announcement fan-out — is
tested at the wire, not in a real client. `tests/e2e/src/comms.rs` proves the
store and the addressing predicate. It does not prove that a message actually
surfaces inside a running Cowork conversation. Treat "an agent gets interrupted
by `@user/handle`" as designed and unverified end to end.

**The APM fix moves historical numbers.** `duration_minutes` used to fall back to
1.0 whenever `ended_at` was null, which scored every crashed or interrupted
session as if all of its work happened in a single minute. It is now
`COALESCE(ended_at, last_event_at)`. This is a correctness fix, but every
historical `apm` / `eapm` rollup changes value as a result. If anyone has
screenshotted or reported those numbers, they will no longer reconcile. Nothing
is versioned or backfilled — the new query simply reads the old rows differently.

**`check-fork-drift` fails and is masking clippy.** `just clippy` and
`just preflight` both run `lint-gates` first and abort on its failure, and
`check-fork-drift` fails on an untouched HEAD (it needs `SIBLING_REPO` set).
The consequence is that **clippy is silently skipped** — the recipe exits
non-zero for a reason that has nothing to do with lint, and a reader sees a red
gate rather than a clippy result. This masked two real failures today. Until it
is fixed, run the compilers directly; see *What to re-run*.

**Per-crate `.sqlx` caches now regenerate far larger than they are tracked.**
`just prepare` runs a per-crate `cargo sqlx prepare` for each dir in
`EXTENSION_DIRS`, and that caches every query reachable in the crate's
dependency graph — not just the crate's own. `extensions/mcp/comms` comes out
clean (11 queries, 11 files) because it depends on core narrowly. But
`extensions/mcp/shared` moved to `systemprompt = { features = ["full"] }` in
`e5f7bf5e` so the artifact theme could reach `systemprompt::mcp`, and its
regenerated cache is now **675 files against 86 tracked**. `extensions/mcp/email`
regenerates 681 against the 5 it actually needs.

Both are committed pruned to their own queries, which is the existing convention
and what `comms` already does. The consequence to know about: **`just prepare`
now leaves ~1250 untracked files behind**, and they look exactly like a cache you
forgot to commit. They are not. `scripts/check-sqlx-cache.sh` only checks that a
cache is *present*, so it will not catch this either way — the real enforcement
is the offline build. If you prepare and see hundreds of untracked entries under
`extensions/mcp/{shared,email}/.sqlx/`, prune to the crate's own queries rather
than committing them.

Also fixed in passing: `extensions/mcp/email` was missing from `EXTENSION_DIRS`
entirely, so the recipe that exists to generate that crate's cache never ran for
it.

**`just e2e` silently reuses a stale MCP binary.** It builds the server binaries
only when none exists, so an old one is spawned unchanged. This produced a
failure on this tree that read as a genuine linking defect and was not — details
and the fix in *What to re-run*. It is listed here because the failure message it
produces is convincing, and the next person to see it will believe it.

**Core's `require_approval` has no enforcement point of its own.** Covered in
*What landed* — repeated here because it is the single most misleading thing
about this feature. The hold works in this installation because
`extensions/mcp/shared/src/approval/` implements it. It does not work in core,
and `approval_requests` has been in core's schema since migration 014 without
one. Documented in core's `bee179e99`, and corrected in `f4e907d59`'s body — the
first pass concluded the feature was dead everywhere, which is wrong for the
reason above. Read the correction before quoting either commit.

**The comms hook may destroy the inbox it was meant to deliver.** The bridge
registers it on both `UserPromptSubmit` (sync) and `Stop` with `is_async: true`.
An async hook's stdout is not consumed — so a `Stop` firing first drains the
inbox, which is destructive, and surfaces nothing. Nobody has decided whether
`Stop` should be registered at all. Untested and unresolved; see core
`59fc30e59`.

**`quality.yml`'s `msrv` job had never tested MSRV — fixed in `a99d137d`.** It
installed 1.94.0 via `dtolnay/rust-toolchain@1.94.0` and then ran a bare
`cargo check`, but `rust-toolchain.toml` pins `nightly-2026-06-03` and a
toolchain file overrides rustup's default — so the check ran that nightly every
time. Confirmed in this tree: a bare `cargo` reports 1.98.0-nightly,
`cargo +1.94.0` reports 1.94.0.

Now uses `+1.94.0` and asserts the toolchain actually in use before checking.
The assertion is the durable half: the defect was never a wrong answer, it was a
job that passed while proving nothing, and only an explicit check can fail that
loudly. This workspace does satisfy 1.94 — verified with a real
`cargo +1.94.0 check --workspace` — so the job went honest without going red. The
local `just msrv-check` recipe was always correct; only CI made an untested
claim.

Core's equivalent job is a different problem and is being fixed there: it checks
both the root workspace and `bin/bridge` against 1.94, but `bin/bridge/Cargo.toml`
declares no `rust-version` at all, so the job invented a claim the bridge never
made. Root's `rust-version = "1.94"` is a real published promise for the 33
crates that go to crates.io and genuinely passes; the bridge needs 1.95 for
vergen-gitcl, and no 10.x release works on 1.94 (10.0.0/10.0.1 declare 1.95,
10.0.2/10.0.3 declare 1.96, and it is a build-dependency, so it constrains who
can build the bridge rather than who can consume the crates). The fix there is to
give each workspace its own declared MSRV, not to move a public claim.

**`quality.yml`'s `file-size` job cannot fail** (core only — this repo has no
such job).** Its recipe pipes `find` into
`awk` and prints; `awk` exits 0 whether or not it printed anything. Forty-nine files
are over the 300-line guard right now, under a green tick — all hand-written
source; the recipe already excludes `target/` and `tests/` and discounts `//!`
heads, so the number is not inflated. That is consistent
with the house rule that file size is informational — but the job's presence
implies a gate that does not exist, which is the same shape as the `msrv` defect
above and the self-skipping e2e test below.

Deliberately unresolved, and the reason is worth recording: the two honest fixes
point in opposite directions. Making the gate real (a baseline plus ratchet, as
`coverage/baseline.json` already does here) contradicts the standing guidance
that file size is informational rather than a blocker — it would go red the day
someone writes a 301-line file. Removing the job admits it was never a gate. That
is a call for a person, not for a quality pass, and it is with Ed.

**`next` has never been gated.** This one stopped being theoretical today: core's
first gate cycle went red on `bridge-native` clippy on windows-latest —
`unused_qualifications` on `egress::cowork_egress_allowed_hosts()`
(`install/mdm/mod.rs:172`, where line 15 already re-exports it unqualified),
introduced by `ac8eda7d0`, which had sat on `next` ungated. It is also
runner-only: reproducing it locally with
`--target x86_64-pc-windows-msvc` fails before clippy runs, because `ring`'s
build script needs a real MSVC toolchain. So local checks could not have caught
it at any diligence.

Neither this repo's CI nor Quality workflows trigger on pushes to `next` at all — they run on PRs and on `main`. Every commit
on `next` since the last promote, including all of today's, has been validated
only by whatever a person chose to run locally. This is a repo-level gap, not a
property of today's work, and it is the reason the *What to re-run* section
exists in this document at all.

**Authorship was collapsed.** Four agent sessions produced this work
concurrently and all four exited before it was committed. It was committed by a
fifth under one identity. The commit bodies name what came from where, and that
prose is now the only record — the original per-session provenance survives
solely in this machine's session transcripts. If provenance matters for any of
this code, recover it now rather than later.

---

## 5. What to re-run

`just clippy` and `just preflight` do not do what their names say (see
*Known-weak spots*). Run this instead. Both workspaces — the root workspace
passing proves nothing about `tests/`, which is a separate workspace; adding a
field to a pub struct in `extensions/web/admin` broke `tests/integration/admin-core`
today while the root stayed green.

```bash
cargo fmt --all -- --check
bash scripts/check-sqlx-cache.sh
SQLX_OFFLINE=true cargo clippy --workspace --all-targets --all-features -- -D warnings
SQLX_OFFLINE=true cargo clippy --manifest-path tests/Cargo.toml --workspace --all-targets -- -D warnings
SQLX_OFFLINE=true RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
just msrv-check
just lint-gates                      # expect only check-fork-drift
cargo nextest run --manifest-path tests/Cargo.toml \
    -p mcp-unit-tests -p web-unit-tests -p email-unit-tests

# e2e: rebuild the MCP binaries first, and bound the concurrency. Both matter —
# see the two notes below.
cargo build -p systemprompt-mcp-odoo -p systemprompt-mcp-email -p systemprompt-mcp-comms
export SYSTEMPROMPT_TEST_DATABASE_URL=postgres://systemprompt:123@localhost:5448/postgres
cargo nextest run --manifest-path tests/Cargo.toml -p e2e-tests -j 4
```

Last run on this tree: **543 unit tests passed**, **23/23 e2e passed**, everything
above green except `check-fork-drift`.

On `systemprompt-core` at `f4e907d59`: 19,667 tests across all 13 shards green
against fresh migrated databases, plus rustfmt, clippy `-D warnings` and rustdoc
`-D warnings` on all three workspaces, all 15 source-gate lints, machete, deny,
sqlx-verify-offline and `cargo build --workspace --locked`. Its hosted CI,
Quality and Supply Chain gates all report `success`, each verified by its own
conclusion and `headSha` at `f4e907d59` rather than by a summary line.

The full ladder core was run through is `internal/quality-check.md` in that repo
(gitignored). It exists because `release-flow.md` §3 listed eight commands while
`quality.yml` alone runs twenty-one — eleven gates were missing from the
documented procedure, including every one that caught something today.

**`just e2e` reuses a stale MCP binary.** The recipe builds
`systemprompt-mcp-odoo` / `-email` only `if [ ! -x ... ]` — if a binary exists at
all, however old, it is spawned as-is. On this tree that produced a real,
correct-looking failure:
`the_crm_table_arrives_as_a_branded_ui_resource` panicked with *"the shipped
binary renders unbranded — the ArtifactTheme registration did not survive
linking"*. The registration was fine; the binary on disk predated it by a day and
contained zero theme tokens (`strings target/release/systemprompt-mcp-odoo |
grep -c '0.67 0.18 50'` → 0; a fresh build → 4). The harness already picks
newest-by-mtime because this bit someone once before (`harness/mcp.rs:33`), but
the *build* side of the recipe still has the hole. Build the binaries explicitly.

**The suite cannot run at full parallelism against one Postgres.** All 23 tests
each assemble a full production `AppContext` with its own pool. Run unbounded,
three of them died at ~59s with
`assemble the production AppContext: Repository(Database(PoolTimedOut))` —
which looks like a code failure and is not. `-j 4` completes in ~93s, all green.

Then the same clippy pair in `../systemprompt-core`.

**One trap worth naming explicitly.** `Stack::create()` returns `None` and the
test silently returns green when `SYSTEMPROMPT_TEST_DATABASE_URL` is unset —
no failure, no warning, just a pass in hundredths of a second. A DB-backed e2e
test really takes 11–24s. If the whole suite finishes instantly, you have proved
nothing.

Read the timings rather than the count, but know the two legitimate exceptions:
`artifact_gallery::every_artifact_type_renders_with_the_brand_theme` (~0.05s) and
`skills_artifacts::the_skill_artifact_bundles_match_their_source` (~0.2s) never
touch `Stack::create()` and are genuinely fast. Every other sub-second pass in
this suite is a skip.
