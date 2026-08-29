# Set Up the Control Plane

Install the control-plane dashboards — the user directory, request activity, and usage and costs —
into the Artifacts library, and confirm the admin CLI server answers. Safe to run on every new
session: it reconciles rather than seeds, so re-running is the point, not a waste.

**This skill is admin-only, and the grant is what enforces it.** It ships in the
`systemprompt-admin` plugin, which `services/access-control/roles.yaml` grants to `[admin]` with
`default_included: false`, so it never appears in a non-admin's signed manifest and its bundle is
never mounted for them. Nothing in this file re-checks the role; by the time you are reading it, the
check has already passed.

**Run `systemprompt_setup_cowork` first.** That skill installs the workspace dashboards (the four
Odoo pages) — which admins use daily too. This one adds the control plane on top. The two do not overlap: each stages only its own bundles.

## Before you start — this skill only works in Claude Cowork

The Artifacts library is a Cowork feature. You need a **`create_artifact` tool that takes an
`html_path`**, a `list_artifacts` tool, and a session `outputs/` directory. If any is absent, stop
and say so. Do **not** stage HTML "in case", do not write a receipt, and do not look for a CLI to
install artifacts with — there is no `coworkctl`, no `cowork` command, and no HTTP endpoint for
this. A receipt reporting `installed: 0` on a host with no library is a wrong answer dressed as a
result.

## Ask me things like

- "Set up my admin dashboards."
- "Install the control plane."
- "Are the governance dashboards installed?"

## Step 1 — Stage the admin bundle

Run the shared staging script **once**, restricted to the admin bundle (it finds the script wherever
the skills mount lands — do not substitute a guessed path, and never a Windows path):

```
SETUP=$(find "$HOME/mnt" /sessions/*/mnt -name setup.sh -path '*systemprompt?setup*' 2>/dev/null | head -1) \
  && sh "$SETUP" -- systemprompt-admin || echo "SETUP_SCRIPT_NOT_FOUND"
```

The `-- systemprompt-admin` argument is what keeps this run to the control plane: without it the
script stages every bundle you have mounted and you would reinstall the workspace dashboards too.

It copies each dashboard page into the session `outputs/` directory and prints `OUTPUTS_DIR=`,
`PLUGINS=`, `COPIED=`, then one ready-made `create_artifact` parameter block per record — `id`,
`description`, `html_path`, `mcp_tools`, plus a comment with `name`/`starred`/`version` — and finally
`TOTAL_RECORDS=`. That printed set **is** the bundled set: count it, never assume it. Expect
`admin-users-directory`, `admin-activity-requests`, and `admin-usage-costs`.

Do not read the `.html` files into context — they are large, and they get copied, not retyped.

## Step 2 — Diff bundled against installed

List the artifacts already in the library and match **by `id` only** — never by name, title, or
"close enough" description. `Activity — Recent Requests` and `Recent Activity — Team Notes` are
different dashboards from different bundles, and a name match is exactly how one ends up installed
with the other's tool allowlist. Build four groups:

- **Missing** — bundled but not in the library.
- **Present** — bundled and already there.
- **Stale** — present, but the bundled `version` differs from what was installed.
- **Superseded** — a library entry with one of the retired ids `admin-users`, `admin-activity`,
  `admin-usage`: an install from before these dashboards were renamed. Offer to remove it; the
  bundled dashboards are its replacement and it can no longer load data.

Leave every non-`admin-*` entry alone. It belongs to `systemprompt_setup_cowork` and is not yours to
reconcile here.

## Step 3 — Install what is missing

For each **missing** record, call the built-in `create_artifact` tool with exactly the printed block
— sequentially, never in parallel. Include the `mcp_tools` list every time: without it the dashboard
cannot call its MCP server and will never load data.

**Verify** with one `list_artifacts` after the batch: every bundled id must appear, **and each
installed record must carry the same tool allowlist as its manifest record** — compare the listed
`mcp_tools` against the record's `mcpTools` for that id, verbatim. A mismatch renders but every
fetch fails with "not in this artifact's mcp_tools allowlist", so it is a failed install: delete
that one artifact, re-run its `create_artifact` from the printed block, and re-verify. An artifact
counts as installed only when the listing shows it with the right allowlist — never because the
create call "should have" worked.

For each **stale** record, say it is out of date and offer to replace it. Never silently overwrite
an artifact the user may have edited.

**Caching contract:** Cowork caches a dashboard's MCP results only when the tool advertises
`annotations.readOnlyHint: true`. The admin CLI tool (`mcp__systemprompt__systemprompt`) is
deliberately *not* annotated, so these three dashboards are never cached and always refetch on
render. That is intended — control-plane numbers must not be stale — and it is why they feel slower
than the workspace pages.

## Step 4 — Check the admin connection

Run one read-only probe against the admin CLI server, as the signed-in user:

| server | probe |
|--------|-------|
| `systemprompt` | `systemprompt` with `{ "command": "core skills list" }` |

Call it by its full wire name, `mcp__systemprompt__systemprompt`. The server is gated three ways —
the `systemprompt-admin` plugin grant, its own `entity_type: mcp_server` rule in `roles.yaml`, and
`oauth.scopes` in `services/mcp/systemprompt.yaml` — so a failure here means one of those three, not
a broken dashboard. Report which, and do not retry the call in a loop.

The dashboards fetch their own data when opened, so a working probe means they will populate.

## Step 5 — Write a receipt

Write it through the same script so the timestamp is real, not typed:

```
SETUP=$(find "$HOME/mnt" /sessions/*/mnt -name setup.sh -path '*systemprompt?setup*' 2>/dev/null | head -1) \
  && sh "$SETUP" receipt '{ "checkedAt": "__NOW__", "plugins": ["systemprompt-admin"], "bundled": N,
  "installed": N, "created": ["..."], "alreadyPresent": ["..."], "stale": ["..."],
  "superseded": ["..."], "failed": [] }'
```

`bundled` is `TOTAL_RECORDS=`; `installed` is the count confirmed by the final library listing and
nothing else. The script replaces `__NOW__` with the current UTC time and writes
`outputs/setup-receipt.json` — never write into the plugin or skills directories, which are
read-only and replaced wholesale on every sync. This overwrites the workspace run's receipt, which
is expected: it is a per-run record, not a log. If the write fails, report the same JSON inline
rather than failing the run.

## Step 6 — Report honestly

State plainly **"N of M control-plane dashboards installed"**, N from the verified listing — never a
number you did not verify, and report a partial result as partial. Then list what was created, what
was already there, anything stale, superseded, or failed, and whether the admin CLI answered.
If everything was already present, say so — that is a successful run, not a no-op.

Finish by pointing at the two places these dashboards do not cover: `/admin/governance/approvals`,
where held tool calls wait for a named human, and `/admin/access/users`, where roles are granted.
