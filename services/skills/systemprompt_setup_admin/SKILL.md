# Set Up the Control Plane

Install every dashboard the bridge staged — all five — into the Artifacts library, retire the
dashboards they replaced, and confirm the admin CLI server answers. That is the two workspace pages
every role's data feeds (**My Day**, **Sales Pipeline**) plus the three control-plane pages (request
activity, spend by model, and the governance approvals queue). Safe to run on every new session: it reconciles rather than seeds, so
re-running is the point, not a waste.

**These dashboards write, not just read.** Ticking an activity, moving a lead to another
stage, closing a deal won or lost, logging a note, scheduling a follow-up — each is a real Odoo
write, executed as the signed-in user against their own login, so Odoo's own record rules are the
authorisation boundary. **Governance — Approvals** writes too, and further: approving a held call
releases it to run, and the decision is stamped with the approver's name on the audited row. That is why the tool allowlist matters more than it used to; Step 3 says
what a mismatch costs now.

**Installing dashboards is an admin job, and this is the only skill that does it.** There is no
user-facing setup skill; a non-admin has nothing to run and nothing to install. If a non-admin asks
why their library is empty, the answer is that an admin installs dashboards, not that something is
broken.

**This skill is admin-only, and the grant is what enforces it.** It ships in the
`systemprompt-admin` plugin, which `services/access-control/roles.yaml` grants to `[admin]` with
`default_included: false`, so it never appears in a non-admin's signed manifest. Nothing in this
file re-checks the role; by the time you are reading it, the check has already passed. **Never
diagnose a tool failure as a role, permission or governance problem** — a governance denial arrives
as an explicit verdict naming the policy, never as a generic tool error.

**This skill uses no shell.** Everything it needs is a file the bridge has already placed in your
connected workspace, and the only tools it calls are `Glob`, `Read`, `Write`, `list_artifacts`,
`create_artifact` and the admin CLI server. Do not call `mcp__workspace__bash`; Claude Desktop
denies it in Cowork sessions on current builds, and nothing here needs it.

## Ask me things like

- "Set up my admin dashboards."
- "Install the control plane."
- "Are the governance dashboards installed?"

## Step 1 — Find the bundle the bridge staged

On every sync the bridge stages the dashboard bundle into the pre-trusted workspace folder that
every Cowork session has connected. It goes to **one deterministic absolute path**: the bridge
joins the home directory with `Systemprompt`, then `systemprompt/artifacts`.

```
Windows   %USERPROFILE%\Systemprompt\systemprompt\artifacts\
macOS     ~/Systemprompt/systemprompt/artifacts/
```

`manifest.json` sits in that directory, with one `<id>.html` page beside it per record, verbatim.

### Finding it — try these in order, and stop at the first hit

1. **Read the absolute path.** `Read` `manifest.json` at the platform path above. This is the cheap,
   definitive test and it is right almost every time. A successful read ends discovery — do not glob
   at all.
2. **Glob rooted at the workspace folder.** Only if the read failed: `Glob` for `manifest.json` with
   that artifacts directory as the search root; if that misses, `Glob` for
   `Systemprompt/**/artifacts/manifest.json` from the home directory.
3. **Glob unrooted.** Last, `Glob` for `**/systemprompt/artifacts/manifest.json`, as the catch-all
   for a workspace mounted somewhere unexpected.

A recursive `**/` glob is rooted at this session's own working directory, and a connected folder is a
separate root it is not guaranteed to reach — which is why the glob is the fallback and the absolute
path is the primary. **A miss on any single rung is not evidence that the bridge has not synced.**
Only report a sync problem after every rung above has missed and the directory probe below tells you
which state you are in. Never send the user to press **Sync** on the strength of one failed glob.

Once you have it, `Read` the manifest. It is small: one record per dashboard —
`{ "id", "name", "description", "version", "isStarred", "mcpTools": [...], "plugins": [...] }`.
The pages are not in it and you never read them: `create_artifact` copies a page from its path.

**Take every record in the manifest.** Do not filter on `plugins` — the manifest holds exactly the
bundles your signed manifest granted, so every record in it is one you are meant to install. That
printed set **is** the bundle: count it, never assume it. On this instance expect five — the two
workspace pages (`my-day`, `sales-pipeline`) and the three control-plane pages
(`admin-activity-requests`, `admin-usage-costs`, `governance-approvals`). If the count differs, install what is there and
say so — the manifest is the authority, not this list.

### If every rung missed

Probe the directories before naming a cause, and report the one that matches:

- **The artifacts directory exists but holds no `manifest.json`** — a partial or interrupted sync.
  Say that, and ask the user to press **Sync** in the bridge.
- **`Systemprompt/` exists but has no `systemprompt/` inside it** — the folder is connected and the
  bridge has never completed a sync. Say that, and ask them to press **Sync**.
- **No `Systemprompt/` folder at all** — on Windows the bridge creates it at install, so this means
  the bridge is not installed or its policy write did not apply; on macOS the folder only appears at
  the first sync, so it means no sync has ever run. Name the cause for the host you are on.
- **You could not probe the paths at all** (no readable home directory, or the file tools refused) —
  say discovery could not run, and say plainly that this is *not* evidence about sync state.

If the session shows no connected folder at all, tell the user to connect the `Systemprompt` folder.
Then stop. Do not look for a script, do not copy pages by hand, and do not report a zero-install
receipt.

## Step 2 — Diff bundled against installed

Call `list_artifacts` and match **by `id` only** — never by name, title, or "close enough"
description. A name match is exactly how a dashboard ends up installed with another's tool
allowlist, and the names here are close enough to invite it: `Activity — Recent Requests` is the
control-plane request log, while `My Day` carries the team's activity feed. Build four groups:

- **Missing** — bundled but not in the library.
- **Present** — bundled and already there.
- **Stale** — present, but the bundled `version` differs from what was installed.
- **Superseded** — a library entry carrying one of the retired ids below. Offer to remove it: it
  can no longer load data, and one of the bundled dashboards is its replacement.

  | retired id | replaced by |
  |---|---|
  | `todo-bulletin`, `recent-activity`, `business-overview` | `my-day` |
  | `pipeline-open-deals`, `upcoming-deals`, `leads-inbound-prospects` | `sales-pipeline` |
  | `admin-users`, `admin-activity`, `admin-usage` | the renamed `admin-*` pages |
  | `quotes-and-invoices`, `admin-users-directory`, `knowledge-feed`, `knowledge-approve-ingestion` | nothing — retired outright |

  The last row has no replacement, so say so plainly rather than pointing at a substitute. Removing
  `knowledge-feed` and `knowledge-approve-ingestion` changed nothing about the brain@ pipeline
  itself: it still runs, and its proposals are still decided in chat or at
  `/admin/governance/approvals`.

  Name the retired ids and stop there. Do not append what a removed page "would have" covered, or
  which surface now owns a job nobody asked about — the reader is removing stale records, not
  reading a tour of the admin area.

Leave every library entry that is not in the manifest alone. It may be the user's own artifact, and
nothing here owns it.

## Step 3 — Install what is missing

For each **missing** record, call `create_artifact` — sequentially, never in parallel — with:

- `id` and `description` from the record,
- `html_path` set to the page beside the manifest, spelling the directory exactly as the rung that
  found the manifest spelled it: `<artifacts dir>/<id>.html`. That folder is connected, so the path
  is accepted as it is. Never point at the plugin or skills directories, and never retype a page.
- `mcp_tools` set to the record's `mcpTools`, verbatim. Without it the dashboard cannot call its
  MCP server and will never load data.
- `name` and `starred` from the record if the tool's schema exposes such fields; otherwise skip
  them silently.

**Verify** with one `list_artifacts` after the batch: every bundled id must appear, **and each
installed record must carry the same tool allowlist as its manifest record** — compare the listed
`mcp_tools` (however the library names the field) against the record's `mcpTools`, verbatim. A
mismatch renders but every fetch fails with "not in this artifact's mcp_tools allowlist", so it is
a failed install: delete that one artifact, re-run its `create_artifact`, and re-verify. An artifact
counts as installed only when the listing shows it with the right allowlist.

For each **stale** record, say it is out of date and offer to replace it. Never silently overwrite an
artifact the user may have edited.

**Caching contract:** Cowork caches a dashboard's MCP results only when the tool advertises
`annotations.readOnlyHint: true`. The admin CLI tool (`mcp__systemprompt__systemprompt`) is
deliberately *not* annotated, so the two control-plane dashboards always refetch on render. That
is intended — control-plane numbers must not be stale. The Odoo read tools do carry the annotation
and are cached normally; the Odoo *write* tools do not, which is correct — a write must never be
served from a cache.

## Step 4 — Check the admin connection

Run one read-only probe against the admin CLI server, as the signed-in user:

| server | probe |
|--------|-------|
| `systemprompt` | `systemprompt` with `{ "command": "core skills list" }` |

Call it by its full wire name, `mcp__systemprompt__systemprompt`. The server is gated three ways —
the `systemprompt-admin` plugin grant, its own `entity_type: mcp_server` rule in `roles.yaml`, and
`oauth.scopes` in `services/mcp/systemprompt.yaml` — so a failure here means one of those three, not
a broken dashboard. Report which, and do not retry the call in a loop.

## Step 5 — Write a receipt

`Write` this JSON to `outputs/setup-receipt.json` in the session's outputs directory, with the
current UTC time in `checkedAt`:

```
{ "checkedAt": "<ISO-8601 UTC>", "plugins": ["<every plugin named across the manifest records>"],
  "bundled": N, "installed": N,
  "created": [{ "id": "...", "name": "...", "ref": "..." }],
  "alreadyPresent": [{ "id": "...", "name": "...", "ref": "..." }],
  "stale": ["..."], "superseded": ["..."], "failed": [] }
```

`bundled` is the full record count from Step 1; `installed` is the count confirmed by the final
listing and nothing else. `ref` is whatever identifying reference `create_artifact` or
`list_artifacts` exposed for that record (an id, a url — read what the tool gives back). Never write
into the plugin or skills directories, or into the bundle folder: they are replaced wholesale on
every sync. If the write fails, report the same JSON inline rather than failing the run.

## Step 6 — Report honestly

State plainly **"N of M dashboards installed"** (M from the manifest, never assumed; N from the
verified listing). List what was created and what was already there by name with its reference,
anything stale, superseded, or failed, and whether the admin CLI answered. If everything was already
present, say so — that is a successful run, not a no-op.

Finish by pointing at the two places these dashboards do not cover — both of which used to have a
dashboard of their own and deliberately no longer do: `/admin/governance/approvals`, where held tool
calls and brain@ ingestion proposals wait for a named human, and `/admin/access/users`, where roles
are granted and the user directory lives.
