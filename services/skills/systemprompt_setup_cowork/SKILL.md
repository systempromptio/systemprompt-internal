# Set Up My Workspace

Bring the Artifacts library into line with the dashboards your plugins ship: install any that are
missing, leave the ones already there alone, and report a clear "installed X of Y" result. Safe to
run on every new session — it reconciles rather than seeds, so re-running is the point, not a waste.

**This skill installs the workspace dashboards only** — the four Odoo pages (business overview,
inbound leads, open pipeline, recent activity) from the `systemprompt-user` bundle. It does not touch the control-plane dashboards (users, activity, usage): those belong to
`systemprompt_setup_admin`, a separate skill that only admins hold. If you are an admin, run this
skill first for your workspace, then that one for the control plane. If you are not, that skill does
not exist for you and nothing here is missing.

## Before you start — this skill only works in Claude Cowork

The Artifacts library is a Cowork feature. Check for it before doing anything else: you need a
**`create_artifact` tool that takes an `html_path`**, a `list_artifacts` tool, and a session
`outputs/` directory. If any of those is absent, **stop here** and hand over:

- **Codex CLI** — follow `systemprompt_setup_codex` instead. Codex has no artifact library at all,
  and its inline visualizations are blocked from calling MCP tools (`callMcp` rejects with "Inline
  visualizations cannot call tools"), so a dashboard could not load data even if you rendered one.
- **Plain Claude Code or any other MCP client** — nothing to install. Verify one MCP server answers
  (`crm_lead_search` with `{ "limit": 1 }`) and stop.

Stopping means stopping. Do **not** stage the HTML into `outputs/` "in case", do not write a
receipt, and do not look for a CLI to install artifacts with — **there is no `coworkctl`, no
`cowork` command, and no HTTP endpoint for this.** `create_artifact` is a built-in tool or it is
nothing. A receipt reporting `installed: 0` on a host that has no library is a wrong answer dressed
as a result; say plainly that the skill does not apply here.

## Ask me things like

- "Set up my workspace."
- "Install my dashboards."
- "Are my dashboards installed?"
- "Re-run workspace setup — did anything change?"

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
advertises `annotations.readOnlyHint: true`. Every read-only tool a dashboard calls must carry that
annotation in its server's tool catalog — without it, every re-render refetches and rendering becomes
racy. Every tool the workspace dashboards call carries it.

## Step 1 — Read the manifests the mounted bundles ship

Every mounted plugin bundle lays its dashboards out at its root as `artifacts/manifest.json` —
`{ "artifacts": [ { "id", "name", "description", "version", "isStarred", "mcpTools": [...] }, ... ] }`
— with one `artifacts/<id>.html` beside it per record. Run the staging script in Step 3 first; it
prints every record it found, across every bundle, deduplicated by `id`. That printed set **is** the
bundled set: count it, never assume it. A plugin with no dashboards ships no `artifacts/` directory,
which is normal.

Read **only** the manifests. Do not read the `.html` files into context — they are large, and you
never need their contents: they get copied, not retyped.

## Step 2 — Diff bundled against installed

List the artifacts already in the Artifacts library. Match records to installed ones **by `id`
only**. Never match by name, title, or "close enough" description — several dashboards have
near-homograph names (e.g. "Activity — Recent Requests" vs "Recent Activity — Team Notes"); a name
match is exactly how a dashboard ends up installed with another dashboard's tool allowlist. If the
library genuinely exposes no id for an entry, treat that entry as unmatchable: leave it alone and
report it, never adopt it as one of ours. Build four groups:

- **Missing** — bundled but not in the library.
- **Present** — bundled and already in the library.
- **Stale** — present, but the bundled `version` differs from what was installed.
- **Superseded** — a library entry whose id is one of the retired ids `salesforce-accounts`,
  `salesforce-activities`, `salesforce-cases`, `salesforce-contacts`, `salesforce-leads`,
  `salesforce-opportunities`: an install from before this workspace moved to Odoo. Offer to remove
  it — the bundled dashboards are its replacement, and it can no longer load data. Leave any
  `admin-*` entry alone: it is not yours to reconcile, and `systemprompt_setup_admin` owns it.

## Step 3 — Install what is missing

Run this skill's staging script **once** with this exact shell command (it finds the script wherever
the skills mount lands — do not substitute a guessed path, and never a Windows path):

```
SETUP=$(find "$HOME/mnt" /sessions/*/mnt -name setup.sh -path '*systemprompt?setup*' 2>/dev/null | head -1) \
  && sh "$SETUP" -- '!systemprompt-admin' || echo "SETUP_SCRIPT_NOT_FOUND"
```

The script walks every mounted bundle's `artifacts/` directory, copies every dashboard page into the
session `outputs/` directory, prints `OUTPUTS_DIR=`, `PLUGINS=` (the bundles it found) and
`COPIED=`, then one ready-made `create_artifact` parameter block per record — `id`, `description`,
`html_path`, `mcp_tools`, plus a comment with `name`/`starred`/`version` — and finally
`TOTAL_RECORDS=`.

Then, for each **missing** record, call the built-in `create_artifact` tool with exactly the printed
block — sequentially, never in parallel. Include the `mcp_tools` list every time: without it the
dashboard cannot call its MCP server and will never load data. Also pass the commented
`name`/star values if the tool's schema exposes such fields; if it does not, skip them silently.

**Verify** with one `list_artifacts` after the whole batch: every bundled id must appear, **and for
each one the installed record must carry the same tool allowlist as its manifest record** — compare
the listed `mcp_tools` (however the library names the field) against the record's `mcpTools` for
that id, verbatim. A dashboard installed with another dashboard's allowlist renders but every data
fetch fails with "not in this artifact's mcp_tools allowlist", so a mismatch is a failed install:
delete that one artifact, re-run its `create_artifact` from the printed block, and re-verify. An
artifact counts as installed only when it appears in the list with the right allowlist — never
because the create call "should have" worked. If any create call errored, fix and retry that one
before the final listing.

**Fallback ladder** — take each step only after the previous one provably failed:

1. The find-and-run one-liner above.
2. If it printed `SETUP_SCRIPT_NOT_FOUND`: locate the bundles the same way —
   `find "$HOME/mnt" /sessions/*/mnt -type d -path '*/artifacts' 2>/dev/null`
   — `cat` each `manifest.json` and bash-`cp` every `*.html` beside it into the outputs dir
   (`$HOME/mnt/outputs`, or discover it: `find "$HOME/mnt" /sessions/*/mnt -maxdepth 2 -type d -name outputs`).
3. Only if **both** finds return nothing (the mounts genuinely lack the bundles): Read each plugin
   bundle's `artifacts/<id>.json` (Windows paths, file tools) and Write its `content` string to
   `outputs/<id>.html` **verbatim and unmodified** — no edits, no reformatting, no "improvements" —
   then create as above, and say in the final report that the slow path was used and why.

For each **stale** record, tell the user it is out of date and offer to replace it. Do not silently
overwrite an artifact the user may have edited.

If one artifact genuinely fails after a retry, record it under `failed` and carry on with the rest —
but a workspace-path rejection is not a failure to record, it is a signal you skipped the copy step.

## Step 4 — Write a receipt

Each entry in `created` and `alreadyPresent` is an object, not a bare id — capture whatever
identifying reference `create_artifact`'s response and the `list_artifacts` verification entries
actually expose for that record (an id, a url, however the library names it — same caution as the
`mcp_tools` field in Step 3: read what the tool gives back, do not assume a field name). At minimum
carry `id` and `name`; add `ref` when the tool exposes something beyond the id (a url or a distinct
artifact identifier). This is what lets Step 6 point the user at each dashboard by name instead of
just a count.

Write the receipt through the same script so the timestamp is real, not typed:

```
SETUP=$(find "$HOME/mnt" /sessions/*/mnt -name setup.sh -path '*systemprompt?setup*' 2>/dev/null | head -1) \
  && sh "$SETUP" receipt '{ "checkedAt": "__NOW__", "plugins": ["..."], "bundled": N, "installed": N,
  "created": [{ "id": "...", "name": "...", "ref": "..." }],
  "alreadyPresent": [{ "id": "...", "name": "...", "ref": "..." }],
  "stale": ["..."], "superseded": ["..."], "failed": [] }'
```

`plugins` is the `PLUGINS=` line; `bundled` is `TOTAL_RECORDS=` — count them, never assume.

The script replaces `__NOW__` with the current UTC time and writes
`outputs/setup-receipt.json` (never write into the plugin or skills directories — they are
read-only and replaced wholesale on every sync). If the script is unavailable, write the same JSON
to `outputs/setup-receipt.json` yourself with the timestamp from `date -u`.

`installed` must be the count confirmed by the final library listing, nothing else. If the write
fails, do not fail the run: report the same receipt inline in Step 6 so the result is still visible.

## Step 5 — Check the connections the dashboards need

Collect the distinct MCP servers named across the installed records' `mcpTools` (the part between
`mcp__` and the next `__`) and run one small read-only probe per server, executed as the signed-in
user:

| server | probe |
|--------|-------|
| `odoo` | `crm_lead_search` with `{ "limit": 1 }` |
| `knowledge-bank` | `list_documents` with `{}` |

Call each probe by its **full wire name**, `mcp__<server-id>__<tool>`. The server segment is the id
exactly as `services/mcp/*.yaml` spells it, **hyphens and all** — it is `mcp__knowledge-bank__list_documents`,
never `mcp__knowledge_bank__...`. Nothing normalises a hyphen to an underscore, so the underscore
form is not a near miss that still resolves; it is "No such tool available". Read the name off the
installed record's `mcpTools`, which already carries the correct string, rather than retyping it.

Pass the arguments in the table verbatim. These tools reject unknown keys, so a plausible-looking
extra (`limit` on `list_documents`, which takes only `project` and `source`) fails the probe and
reads as a broken server.

The dashboards fetch their own data when opened (each page calls its MCP tool itself on load, and
Reload re-runs it), so a working probe means the dashboards will populate.

If the Odoo probe fails with an authentication or missing-identity error, say the artifacts are
installed but Odoo is not reachable for this account, and point the user at linking their Odoo login
and API key on `/admin/profile` rather than at the dashboards — the HTML is fine, the credential is
not.

If the Odoo probe fails with **Access Denied**, do not report it as a connection or session problem
— there is no session to refresh, the credential is a long-lived API key and it authenticated fine.
It means the linked Odoo account lacks rights on the model. The error now names the account and the
app; relay that, and point at an Odoo administrator granting it access rather than at relinking.

## Step 6 — Report honestly

never report a number you did not verify, and report a partial result as partial. Name the plugins
the dashboards came from. Then list what was created and what was already there **by name, each with
its Step 4 reference** — not just a count — so the user can jump straight to a dashboard instead of
hunting the Library for it; fall back to naming it plainly if no `ref` was available for that one.
Then note anything stale, superseded, or failed, and which servers answered. If everything was
already present, say so — that is a successful run, not a no-op. Suggest opening **Who Am I** first:
it shows the user their roles, their Odoo link, and exactly which plugins, servers and skills they
were granted — the same grant that decided which dashboards just got installed.
