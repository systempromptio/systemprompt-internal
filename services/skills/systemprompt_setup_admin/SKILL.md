# Set Up the Control Plane

Install the admin bundle's seven dashboards — the two admin business pages (business overview,
inbound leads), the two brain@ knowledge pages (knowledge feed, approve ingestion) and the three
control-plane ones (the user directory, request activity, and usage and costs) — into the Artifacts
library, and confirm the admin CLI server answers. Safe to run on every new session: it reconciles
rather than seeds, so re-running is the point, not a waste.

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

**Run `systemprompt_setup` first.** It installs the user-scoped `systemprompt-workspace` bundle's
four dashboards, which admins hold too. This skill installs the seven that ride only with the admin
bundle. The two do not overlap.

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

Keep only the records whose `plugins` names `systemprompt-admin`. That printed set **is** the admin
bundle: count it, never assume it. Expect `business-overview`, `leads-inbound-prospects`,
`knowledge-feed`, `knowledge-approve-ingestion`, `admin-users-directory`,
`admin-activity-requests` and `admin-usage-costs`.

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
description. `Activity — Recent Requests` and `Recent Activity — Team Notes` are different
dashboards from different bundles, and a name match is exactly how one ends up installed with the
other's tool allowlist. Build four groups:

- **Missing** — bundled but not in the library.
- **Present** — bundled and already there.
- **Stale** — present, but the bundled `version` differs from what was installed.
- **Superseded** — a library entry with one of the retired ids `admin-users`, `admin-activity`,
  `admin-usage`: an install from before these dashboards were renamed. Offer to remove it; the
  bundled dashboards are its replacement and it can no longer load data.

Leave every entry that is not one of the seven above alone — `todo-bulletin`, `upcoming-deals`,
`pipeline-open-deals` and `recent-activity` belong to `systemprompt_setup`.

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
deliberately *not* annotated, so the three control-plane dashboards always refetch on render. That
is intended — control-plane numbers must not be stale.

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
{ "checkedAt": "<ISO-8601 UTC>", "plugins": ["systemprompt-admin"], "bundled": N, "installed": N,
  "created": [{ "id": "...", "name": "...", "ref": "..." }],
  "alreadyPresent": [{ "id": "...", "name": "...", "ref": "..." }],
  "stale": ["..."], "superseded": ["..."], "failed": [] }
```

`bundled` is the admin record count from Step 1; `installed` is the count confirmed by the final
listing and nothing else. `ref` is whatever identifying reference `create_artifact` or
`list_artifacts` exposed for that record (an id, a url — read what the tool gives back). Never write
into the plugin or skills directories, or into the bundle folder: they are replaced wholesale on
every sync. If the write fails, report the same JSON inline rather than failing the run.

## Step 6 — Report honestly

State plainly **"N of M dashboards installed"** (M from the manifest, never assumed; N from the
verified listing). List what was created and what was already there by name with its reference,
anything stale, superseded, or failed, and whether the admin CLI answered. If everything was already
present, say so — that is a successful run, not a no-op.

Finish by pointing at the two places these dashboards do not cover: `/admin/governance/approvals`,
where held tool calls wait for a named human, and `/admin/access/users`, where roles are granted.
