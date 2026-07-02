# Set Up My Workspace

Get a new Astound Digital workspace ready in one run: install the Salesforce dashboards into the
Artifacts library, confirm the Salesforce connection works, and report back what was set up. Safe to
run again at any time — nothing is duplicated.

## Ask me things like

- "Set up my workspace."
- "Install the Astound dashboards."
- "Run first-time setup."
- "Re-run setup — did anything change?"

## Step 1 — Find the staged artifacts

The Astound Bridge syncs a set of ready-made Salesforce dashboards to this machine as one JSON file
per artifact, in a `staging` folder that lives alongside the installed plugins. Look under the
Claude sessions root for this platform:

| Platform | Staging folder |
|----------|----------------|
| Windows | `%LOCALAPPDATA%\Claude-3p\local-agent-mode-sessions\<session>\<org>\cowork_artifacts\staging\` |
| macOS | `~/Library/Application Support/Claude-3p/local-agent-mode-sessions/<session>/<org>/cowork_artifacts/staging/` |
| Linux | `$XDG_CONFIG_HOME/Claude-3p/local-agent-mode-sessions/<session>/<org>/cowork_artifacts/staging/` (default `~/.config/...`) |

There is normally one `<session>/<org>` directory pair; if there are several, use the most recently
modified one. Each file in `staging/` is one artifact record:

```json
{ "id": "...", "name": "...", "description": "...", "version": "...",
  "content": "<html...>", "isStarred": true, "mcpTools": ["..."] }
```

**If the staging folder is missing or empty:** stop and tell the user to open the Astound Bridge
app, sign in, and run a sync — then run this setup again.

## Step 2 — Create each artifact in the library (idempotently)

First list the artifacts already in the Artifacts library. Then, for each `*.json` file in staging:

- **Skip it** if an artifact with the same name already exists in the library — say so briefly.
- Otherwise create a new artifact **natively** (with the built-in `create_artifact` tool) using the
  record's `name`, `description`, and `content` (the HTML, verbatim and unmodified), and **star it**.

Do not edit, reformat, or "improve" the HTML content — it is a finished dashboard that renders and
refreshes itself.

## Step 3 — Check the Salesforce connection

Run one small probe through the Salesforce MCP server — for example a SOQL query like
`SELECT Id, Name FROM Account LIMIT 1`. If it succeeds, the dashboards' Refresh buttons will work.
If it fails, tell the user the artifacts are installed but Salesforce isn't connected yet, and to
check the Salesforce server connection in settings.

## Step 4 — Report

Summarize in plain language: which artifacts were installed, which were skipped as already present,
and whether Salesforce is connected. Suggest opening one of the new dashboards (e.g. "Accounts —
Book of Business") as a first step.
