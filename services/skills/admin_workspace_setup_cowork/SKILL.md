# Set Up Admin Dashboards — Claude Cowork

Bring the Artifacts library into line with the control-plane dashboards this plugin ships: install
any that are missing, leave the ones already there alone, and report a clear "installed X of Y"
result. Safe to run on every new session — it reconciles rather than seeds, so re-running is the
point, not a waste.

This is the admin counterpart to `systemprompt_setup_cowork`, which installs the Odoo CRM dashboards
(leads, activities, pipeline). The two do not overlap: each installs only what its own plugin ships.

## Before you start — this skill only works in Claude Cowork

The Artifacts library is a Cowork feature. Check for it before doing anything else: you need a
**`create_artifact` tool that takes an `html_path`**, a `list_artifacts` tool, and a session
`outputs/` directory. If any of those is absent, **stop here**. Report that the admin dashboards are
a Cowork feature and this host has no Artifacts library, then hand over:

- **Codex CLI** — follow `systemprompt_setup_codex` instead. Codex has no artifact library at all,
  and its inline visualizations are blocked from calling MCP tools (`callMcp` rejects with "Inline
  visualizations cannot call tools"), so a dashboard could not load data even if you rendered one.
- **Plain Claude Code or any other MCP client** — nothing to install. Verify the `systemprompt` MCP
  server answers (`core skills list`) and stop.

Stopping means stopping. Do **not** stage the HTML into `outputs/` "in case", do not write a
receipt, and do not look for a CLI to install artifacts with — **there is no `coworkctl`, no
`cowork` command, and no HTTP endpoint for this.** `create_artifact` is a built-in tool or it is
nothing. A receipt reporting `installed: 0` on a host that has no library is a wrong answer dressed
as a result; say plainly that the skill does not apply here.

## Ask me things like

- "Set up my admin dashboards."
- "Install the control-plane dashboards."
- "Are the admin dashboards installed?"
- "Re-run admin setup — did anything change?"

## How installation works (read this first)

Cowork's `create_artifact` tool does **not** take inline HTML. It takes an `html_path` pointing at a
file that must already sit inside the session workspace — under the session's `outputs/` directory
(or a connected folder). Anything else is rejected with "outside this session's workspace". So every
install is exactly two moves: **get the dashboard's HTML file into `outputs/`, then call
`create_artifact` pointing at it.**

Never retype or reconstruct the HTML yourself, and never call `create_artifact` with a path into the
plugin or skills directories — it will be rejected. Never run multiple `create_artifact` calls in
parallel; install one dashboard at a time, verifying as you go.

**Path rules — these caused every past failure:**

- **bash runs in a Linux VM.** It can never see `C:\...` paths; those work only in the Read/Write
  file tools. Never paste a Windows path into a bash command.
- The VM session directory under `/sessions/` is a **codename** (e.g. `practical-dreamy-dijkstra`),
  not the Windows session UUID — never construct `/sessions/<uuid>` by hand. Always discover
  locations with the find one-liner in Step 3.

**Caching contract:** Cowork only caches a dashboard's MCP tool results when the gateway tool
advertises `annotations.readOnlyHint: true`. The admin CLI tool executes arbitrary commands, so it
is deliberately not annotated and its results are never cached — expect a live fetch on every
render. If a future dashboard calls a dedicated read-only tool instead, annotate it in the server's
tool catalog.

## Step 1 — Read the manifest this skill ships

This skill's own directory contains `assets/artifacts/`, holding:

- `manifest.json` — one metadata record per dashboard:
  `{ "artifacts": [ { "id", "name", "description", "version", "isStarred", "mcpTools": [...] }, ... ] }`
- `<id>.html` — the finished dashboard page for each record.

Expect three: `admin-users-directory`, `admin-activity-requests`, `admin-usage-costs`. Treat
`manifest.json` as the authoritative list and count — if it ships more or fewer, the manifest wins.

Read **only `manifest.json`**. Do not read the `.html` files into context — they are large, and you
never need their contents: they get copied, not retyped.

**If `assets/artifacts/` is missing or empty:** fall back to the plugin bundle — the plugin root
(two levels up from this skill: `../../artifacts/`) carries one `<id>.json` per artifact with the
HTML embedded in a `content` field. If that is missing too, stop and tell the user to open the
Systemprompt Internal bridge app, sign in, run a sync, then start a **new** Cowork session. An empty directory
means the sync has not happened — it is never "nothing to do".

## Step 2 — Diff bundled against installed

List the artifacts already in the Artifacts library. Match manifest records to installed ones **by
`id` only**. Never match by name, title, or "close enough" description — the library also holds the
workspace dashboards, and their names are near-homographs of these (e.g. "Recent Activity — Team
Notes" vs "Activity — Recent Requests"); a name match is exactly how a dashboard ends up installed
with another dashboard's tool allowlist. If the library genuinely exposes no id for an entry, treat
that entry as unmatchable: leave it alone and report it, never adopt it as one of ours. Build three
groups:

- **Missing** — bundled but not in the library.
- **Present** — bundled and already in the library.
- **Stale** — present, but the bundled `version` differs from what was installed.
- **Superseded** — a library entry whose id is one of the retired ids `admin-users`,
  `admin-activity`, `admin-usage`: an older install of the same dashboard under its old name.
  Treat it like stale — offer to replace it with the renamed successor from the manifest.

## Step 3 — Install what is missing

Run this skill's staging script **once** with this exact shell command (it finds the script
wherever the skills mount lands — do not substitute a guessed path, and never a Windows path):

```
SETUP=$(find "$HOME/mnt" /sessions/*/mnt -name setup.sh -path '*admin?workspace?setup?cowork*' 2>/dev/null | head -1) \
  && sh "$SETUP" || echo "SETUP_SCRIPT_NOT_FOUND"
```

The script copies every bundled
dashboard into the session `outputs/` directory and prints `OUTPUTS_DIR=`, then one ready-made
`create_artifact` parameter block per artifact — `id`, `description`, `html_path`, `mcp_tools`,
plus a comment with `name`/`starred` values.

Then, for each **missing** record, call the built-in `create_artifact` tool with exactly the
printed block — sequentially, never in parallel. Include the `mcp_tools` list every time: without
it the dashboard cannot call the admin MCP server and will never load data. Also pass the commented
`name`/star values if the tool's schema exposes such fields; if it does not, skip them silently.

**Verify** with one `list_artifacts` after the whole batch: every bundled id must appear, **and for
each one the installed record must carry the same tool allowlist as its manifest record** — compare
the listed `mcp_tools` (however the library names the field) against the manifest's `mcpTools` for
that id, verbatim. A dashboard installed with another dashboard's allowlist renders but every data
fetch fails with "not in this artifact's mcp_tools allowlist", so a mismatch is a failed install:
delete that one artifact, re-run its `create_artifact` from the printed block, and re-verify. An
artifact counts as installed only when it appears in the list with the right allowlist — never
because the create call "should have" worked. If any create call errored, fix and retry that one
before the final listing.

**Fallback ladder** — take each step only after the previous one provably failed:

1. The find-and-run one-liner above.
2. If it printed `SETUP_SCRIPT_NOT_FOUND`: locate the assets the same way —
   `find "$HOME/mnt" /sessions/*/mnt -type d -path '*admin?workspace?setup?cowork/assets/artifacts' 2>/dev/null | head -1`
   — and bash-`cp` every `*.html` from it into the outputs dir (`$HOME/mnt/outputs`, or discover it:
   `find "$HOME/mnt" /sessions/*/mnt -maxdepth 2 -type d -name outputs`).
3. Only if **both** finds return nothing (the mounts genuinely lack this skill): Read the plugin
   bundle's `artifacts/<id>.json` (Windows paths, file tools) and Write its `content` string to
   `outputs/<id>.html` **verbatim and unmodified** — no edits, no reformatting, no "improvements" —
   then create as above, and say in the final report that the slow path was used and why.

For each **stale** record, tell the user it is out of date and offer to replace it. Do not silently
overwrite an artifact the user may have edited.

If one artifact genuinely fails after a retry, record it under `failed` and carry on with the rest —
but a workspace-path rejection is not a failure to record, it is a signal you skipped the copy step.

## Step 4 — Write a receipt

Write the receipt through the same script so the timestamp is real, not typed:

```
SETUP=$(find "$HOME/mnt" /sessions/*/mnt -name setup.sh -path '*admin?workspace?setup?cowork*' 2>/dev/null | head -1) \
  && sh "$SETUP" receipt '{ "checkedAt": "__NOW__", "bundled": N, "installed": N,
  "created": ["..."], "alreadyPresent": ["..."], "stale": ["..."], "failed": [] }'
```

`bundled` is the number of records in `manifest.json` — count them, never assume.

The script replaces `__NOW__` with the current UTC time and writes
`outputs/admin-setup-receipt.json` (never write into the plugin or skills directories — they are
read-only and replaced wholesale on every sync). If the script is unavailable, write the same JSON
to `outputs/admin-setup-receipt.json` yourself with the timestamp from `date -u`.

`installed` must be the count confirmed by the final library listing, nothing else. If the write
fails, do not fail the run: report the same receipt inline in Step 6 so the result is still visible.

## Step 5 — Check the admin CLI connection

The dashboards fetch through the `systemprompt` MCP server, which requires the admin role.
Run one small probe — `core skills list` — through that server. The dashboards fetch their own data
when opened (each page calls the MCP tool itself on load, and the header Reload button re-runs it),
so a working probe means the dashboards will populate.

If it fails with an authorization error, say the artifacts are installed but the admin CLI server is
not reachable for this account, and point the user at their role assignment rather than at the
dashboards — the HTML is fine, the permission is not.

## Step 6 — Report honestly

State plainly: **"N of M dashboards installed"**, where N comes from the verified library listing —
never report a number you did not verify, and report a partial result as partial. Then list what was
created, what was already there, anything stale or failed, and whether the admin CLI server
answered. If everything was already present, say so — that is a successful run, not a no-op. Suggest
opening one dashboard (e.g. "Users — Directory") as a first step: it loads live data automatically.
