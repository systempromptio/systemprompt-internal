# Set Up My Workspace

Bring the Artifacts library into line with what the Astound Bridge has synced to this machine:
install any dashboards that are missing, leave the ones already there alone, and report a clear
"installed X of Y" result. Designed to be run on every new session — it reconciles rather than
seeds, so running it repeatedly is safe and is the point.

## Ask me things like

- "Set up my workspace."
- "Install the Astound dashboards."
- "Are all my dashboards installed?"
- "Re-run setup — did anything change?"

## Step 1 — Read the artifacts this plugin ships

The dashboards are shipped as one JSON file per artifact, bundled into this plugin by the Astound
Bridge sync. They live in the `artifacts/` directory at the plugin root —
a sibling of the `skills/` directory this file lives in, so relative to this skill they are at
`../../artifacts/`.

Read every `*.json` in that directory. Each file is one artifact record:

```json
{ "id": "...", "name": "...", "description": "...", "version": "...",
  "content": "<html...>", "isStarred": true, "mcpTools": ["..."] }
```

Do not go looking for a staging folder under `Claude-3p`, `local-agent-mode-sessions`, or anywhere
else on the host. That directory is Cowork's own storage on the host machine and is **not mounted
inside this VM** — it is unreadable from here no matter how the path is spelled, and asking the user
to supply the path will not help. The bundled copy above is the supported source.

**If `../../artifacts/` is missing or empty:** stop. Tell the user to open the Astound Bridge app,
sign in, and run a sync, then start a **new** Cowork session so the re-synced plugin is picked up.
Do not treat an empty directory as "nothing to do — all good"; it means the sync has not happened.

## Step 2 — Diff staged against installed

List the artifacts already in the Artifacts library. Match staged records to installed ones **by
`id` where the library exposes it, otherwise by exact `name`**. Build three groups:

- **Missing** — staged but not in the library.
- **Present** — staged and already in the library.
- **Stale** — present, but the staged `version` differs from what was installed.

## Step 3 — Install what is missing

For each **missing** record, create the artifact **natively** with the built-in `create_artifact`
tool, using the record's `name`, `description`, and `content` (the HTML verbatim and unmodified),
and star it if `isStarred` is true.

For each **stale** record, tell the user it is out of date and offer to replace it. Do not
silently overwrite an artifact the user may have edited.

Never edit, reformat, or "improve" the HTML — each one is a finished dashboard that renders and
refreshes itself.

If a `create_artifact` call fails, record the failure and carry on with the rest. One broken
artifact must not abort the run.

## Step 4 — Verify and write a receipt

Re-list the library and confirm every bundled `id` is now present. Then write a receipt to
`setup-receipt.json` in the current working directory:

```json
{ "checkedAt": "<ISO-8601 UTC>", "bundled": 6, "installed": 6,
  "created": ["..."], "alreadyPresent": ["..."], "stale": ["..."], "failed": [] }
```

Write it to the session workspace, never back into the plugin directory or anywhere under
`Claude-3p` — the plugin tree is replaced wholesale on every sync, and the host storage is not
writable from inside this VM.

This file is what makes the run auditable — without it there is no record that setup ever
converged. If the write fails, do not fail the run: report the same receipt inline in Step 6 so the
result is still visible.

## Step 5 — Check the Salesforce connection

Run one small probe through the Salesforce MCP server — a SOQL query such as
`SELECT Id, Name FROM Account LIMIT 1`. If it succeeds, the dashboards' Refresh buttons will work.
If it fails, say the artifacts are installed but Salesforce is not connected yet, and point the
user at the Salesforce server connection in settings.

## Step 6 — Report

State plainly: **"N of M dashboards installed"**, then what was created, what was already there,
anything stale or failed, and whether Salesforce is connected. If everything was already present,
say so — that is a successful run, not a no-op. Suggest opening one dashboard (e.g. "Accounts —
Book of Business") as a first step.
