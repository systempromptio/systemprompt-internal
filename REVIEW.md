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

Core's full range for the day is `abbf25b3c..8abc76962`.

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

### Fork convergence and the checks themselves — `52949f3a`, template `c73c010`

Not a feature, but a large part of the day's diff. `check-fork-drift` had failed
all session, masking clippy inside `just clippy` and `just preflight`. 43 shared
files differed unrecorded; 32 are converged and 11 recorded. The template got the
same APM removal, the same MSRV raise, three fixes that originated here, and a
`.fork-divergence` resynced from internal's after rotting to a third of its size.

Three CI checks that could not fail were made real — see *Known-weak spots*, which
is where the interesting part of this work is.

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

**6. Bridge and gateway — core `abbf25b3c..8abc76962`, comms leg in `59fc30e59`**
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

**The APM metric was removed, not fixed.** It read `duration_minutes` as 1.0
whenever `ended_at` was null, scoring every crashed or interrupted session as
though all its work happened in one minute. Correcting that to
`COALESCE(ended_at, last_event_at)` moved every historical `apm`/`eapm` number,
which raised the better question: `apm`, `eapm` and `peak_concurrent` were
written on every Stop event and read by nothing but their own test. All three
columns are gone (internal migration 031; the template's declarative schema),
along with `count_concurrent_sessions`, which existed only to feed
`peak_concurrent`. `analytics::live_sessions::list_live_sessions` covers live
sessions properly. Nothing to reconcile — the numbers are not wrong now, they
are absent.

**`check-fork-drift` was failing and masking clippy — fixed.** `just clippy` and
`just preflight` both run `lint-gates` first and abort on its failure, so a red
fork-drift meant **clippy never ran** and the recipe's failure said nothing about
lint. That masked two real failures during the day.

43 shared files differed with no entry in `.fork-divergence`. 32 are now
converged and 11 recorded with dated reasons; all 21 gates pass in both repos.
Two of the ported fixes were live bugs here, not tidying: the gateway admin UI
dropped `pricing`/`when`/`requires` on every round-trip, silently rewriting
routing policy it does not display, and share tokens had no expiry at all.

The template's own `.fork-divergence` had rotted much further — 27 entries
against internal's 76, with all 10 of its unique entries stale. Divergence is
symmetric, so two lists that disagree just means one stopped being maintained.
It is now internal's list verbatim. It rotted unnoticed because `check-fork-drift`
skips in CI, where only one repo is present: **it is still a local-only gate, and
still the one thing most likely to rot again.**

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

**The MSRV jobs never tested the MSRV — fixed in all three repos.** Each
installed a toolchain and then ran a bare `cargo check`, which silently used the
nightly from `rust-toolchain.toml`. Confirmed by hand: a bare `cargo` reported
1.98.0-nightly where `cargo +1.94.0` reported 1.94.0. The template was worse —
it had no local msrv recipe at all, so nothing anywhere had ever exercised its
claim.

The MSRV is now **1.96** everywhere. The old 1.94 was an unmaintained pin, not a
promise being held to; core's `bin/bridge` had in fact needed 1.95 for
vergen-gitcl all along, against a number it never declared.

`scripts/check-msrv.sh` is byte-identical in internal and the template. It reads
the number from the manifests rather than naming one — a toolchain hardcoded in a
script or a workflow stops matching the day someone edits `rust-version`, which
is this defect exactly — checks the workspaces and `clippy.toml` all agree, uses
`RUSTUP_TOOLCHAIN` (which outranks a toolchain file where installing a toolchain
does not), and asserts the running cargo before checking anything. Core still
hardcodes its toolchain in the workflow; that fails loudly rather than silently,
and matching this script is on core's follow-up list.

**`file-size` could not fail — fixed in core.** Its recipe piped `find` into
`awk` and printed; `awk` exits 0 whether or not it printed, so 49 files sat over
the 300-line guard under a green tick. It is now a real gate, and all 49 files
were split rather than baselined — 146 files changed across four refactor
commits. The gate commit deliberately lands **last**, after the refactors that
satisfy it, so no commit in the series is knowingly red.

The evidence that 49 structural splits changed no behaviour is that the test
count is **identical** before and after (19,667 across 13 shards), and that the
test workspace compiled untouched — every public path the tests import still
resolves, so the re-exports held. This repo's own `check-file-size.sh` was
already a real gate and already passed.

**Four checks were green while proving nothing — read them as one pattern.** This
is the most transferable finding of the day, and it recurred in four independent
places:

  - three MSRV jobs that installed a toolchain and then ran a different one;
  - a `file-size` job whose `awk` exits 0 whether or not it printed;
  - a `require_approval` stage in core with no enforcement point behind it.

A fourth candidate turned out **not** to belong here, and the distinction is the
useful part: the e2e suite does return green in hundredths of a second with no
`SYSTEMPROMPT_TEST_DATABASE_URL`, but `harness/db.rs:29` asserts on exactly that
when `CI` is set, so the suite cannot silently skip where it matters. Verified by
running it with the variable unset and `CI=1`: it panics with "the e2e suite must
not be skipped there". That is a deliberate local-dev affordance with a guard on
the case that counts — not the same defect. The trap it leaves is for a *person*
reading a local run, which is why it stays in *What to re-run* rather than here.

The shape is always the same: **the check names a condition it never asserts.**
Nobody was careless — each looks correct in review, and each passes. What catches
them is asking not "does this pass" but "what does this fail on, and has it ever?"
All three now fixed gained an explicit assertion of the thing they claim, not
just a corrected command, because the corrected command is one edit away from
lying again.

That standard applied back to this repo's own `check-file-size.sh`, which exited
1 correctly but proved nothing about whether it still *could*. It now plants a
301-line file in a temp tree on every run and fails if the scan does not trip on
it, plus a 300-line file that must not trip, so an off-by-one is caught too. Both
arms were verified by sabotaging the script and confirming each one goes red.
Core's file-size gate does not yet do this — it exits 1 on a real violation, but
nothing asserts it can, so a future `| awk` that swallows the status would return
it to silent without looking wrong. **Still open there**; the runbook's current
answer is the habit of introducing a violation by hand, which is weaker than a
mechanism.

**A structural refactor fanned across agents breaks the shared build.** Core's 49
file splits ran as six uncoordinated agents; for about ten minutes the tree did
not compile — symbols left behind in the half that no longer saw them
(`BTreeSet`, a `const`, a `pub(super)`). Each agent fixed its own crate and it
converged, but internal's `[patch.crates-io]` resolves against core's **working
tree**, so an unfinished refactor there breaks builds here. The mitigation is
short windows and a ping, not a lock. Worth knowing before the next big fan-out.

**Verify staging, don't trust the pathspec.** The core refactor was first
committed as one commit sweeping all 145 files under a message describing only
the bridge; it was caught on the `--stat`, soft-reset and redone by area. Nothing
was pushed wrong. Same shape as the `git add -A` hazard in release.md §1d, and
the reason every commit in today's series was staged by explicit path and checked
with `git diff --cached --name-only` first.

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
just msrv-check                      # reads the MSRV from the manifests
SIBLING_REPO=../systemprompt-template just lint-gates   # all 21 should pass
cargo nextest run --manifest-path tests/Cargo.toml \
    -p mcp-unit-tests -p web-unit-tests -p email-unit-tests

# e2e: rebuild the MCP binaries first, and bound the concurrency. Both matter —
# see the two notes below.
cargo build -p systemprompt-mcp-odoo -p systemprompt-mcp-email -p systemprompt-mcp-comms
export SYSTEMPROMPT_TEST_DATABASE_URL=postgres://systemprompt:123@localhost:5448/postgres
cargo nextest run --manifest-path tests/Cargo.toml -p e2e-tests -j 4
```

Last run on this tree: **543 unit tests**, **328 admin-core integration tests**
and **23/23 e2e** all passing, with **all 21 lint gates** green — including
`check-fork-drift`, which had failed all day. The template passes the same 21
gates and its own clippy and MSRV checks.

On `systemprompt-core` at `8abc76962`: 19,667 tests across all 13 shards green
against fresh migrated databases, plus rustfmt, clippy `-D warnings` and rustdoc
`-D warnings` on all three workspaces, all 15 source-gate lints, machete, deny,
sqlx-verify-offline and `cargo build --workspace --locked`. Its hosted CI, Quality and Supply Chain gates all report `success`, each
verified by its own conclusion and `headSha` at `8abc76962` rather than by a
summary line. That Quality run is the first that exercised both previously-fake
jobs, plus `bridge-native` clippy on real Windows and macOS runners — the only
thing that ever compiles the cfg-gated `gui/**` code.

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

**One trap worth naming explicitly — for you, not for CI.** `Stack::create()`
returns `None` and the test silently returns green when
`SYSTEMPROMPT_TEST_DATABASE_URL` is unset — no failure, no warning, just a pass
in hundredths of a second. CI is safe: `harness/db.rs:29` asserts when `CI` is
set and no URL is present, so the suite cannot skip unnoticed there. Locally
nothing stops you. A DB-backed e2e
test really takes 11–24s. If the whole suite finishes instantly, you have proved
nothing.

Read the timings rather than the count, but know the two legitimate exceptions:
`artifact_gallery::every_artifact_type_renders_with_the_brand_theme` (~0.05s) and
`skills_artifacts::the_skill_artifact_bundles_match_their_source` (~0.2s) never
touch `Stack::create()` and are genuinely fast. Every other sub-second pass in
this suite is a skip.
