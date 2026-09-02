# Systemprompt Setup

The one skill for setting up systemprompt.io in whatever host you are running in. It works out who
you are and where you are, then does the setup that host actually supports. Safe to re-run: every
step checks before it changes anything, so re-running reconciles rather than re-seeds.

## Ask me things like

- "Set up systemprompt."
- "Set up my workspace."
- "Install my dashboards."
- "Get me connected to the CRM."
- "Is my systemprompt setup complete?"

## Step 1 — Establish what this account can reach

There is no identity tool on this instance: nothing reports back who the account is or what it was
granted. Establish the two things setup actually depends on, from what you can see:

- **Which skills you were handed.** Your own skill list *is* the grant — the bridge syncs exactly
  what the signed manifest allowed. If `systemprompt_setup_admin` is among them, this is an admin
  account; if it is not, it is a user account, and that skill is not missing.
- **Whether Odoo is linked.** Call `mcp__odoo__crm_lead_search` with `{ "limit": 1 }`. A result means
  linked; an authentication or missing-identity error means it is not — say so and point at
  `/admin/profile` to add an Odoo login and API key. Carry on either way.

State both back in one short sentence before you continue, so the user knows which path they are on.

Never infer a role from an email address or a username. The presence of `systemprompt_setup_admin`
in your own skill list is the only source, because that list is what the manifest actually granted.

## Step 2 — Route on host

| You are running in | Signs | Go to |
|--------------------|-------|-------|
| Claude Cowork | A `create_artifact` tool that takes `html_path`, a `list_artifacts` tool, an `outputs/` directory | **Cowork setup**, below |
| Codex CLI | OpenAI Codex CLI environment, no Cowork artifact tools | **Codex setup**, below |
| Anything else | Plain Claude Code, another MCP client | Nothing host-specific to install. The Odoo check in Step 1 is the whole of setup. Say so and stop. |

**Dashboards are a Cowork feature.** Outside Cowork, never attempt an artifact installation.
`create_artifact` with an `html_path` only exists there, and every past failure came from guessing at
it elsewhere. There is no CLI substitute: **there is no `coworkctl`, no `cowork` command, and no HTTP
endpoint for this.** Do not stage HTML "in case", do not go looking for an install command, and do
not write a receipt reporting zero installs — a receipt reporting `installed: 0` on a host with no
library is a wrong answer dressed as a result. Say plainly that dashboards do not apply on this host
and finish that host's own setup.

---

# Cowork setup

Bring the Artifacts library into line with the dashboards your plugins ship: install any that are
missing, leave the ones already there alone, and report a clear "installed X of Y" result.

This installs whatever dashboards the **user-scoped** bundles you were granted ship. On this
instance that is the `systemprompt-workspace` bundle's four Odoo pages — To-Do Bulletin, Upcoming
Deals, Pipeline — Open Deals, and Recent Activity — Team Notes — which every role holds, so expect
"4 of 4". The admin bundle's seven (business overview, inbound leads, the two brain@ knowledge pages
and the three control-plane ones) belong to `systemprompt_setup_admin`: if Step 1 showed you hold that
skill, run this section, then that one. If you do not hold it, that skill does not exist for you and
nothing here is missing.

## How installation works (read this first)

Cowork's `create_artifact` tool does **not** take inline HTML. It takes an `html_path` pointing at a
file inside the session workspace or a **connected folder**. On every sync the bridge stages the
dashboard bundle into the pre-trusted workspace folder that every Cowork session has connected
(`~/Systemprompt` on this instance):

```
<workspace>/systemprompt/artifacts/manifest.json
<workspace>/systemprompt/artifacts/<id>.html      one page per record, verbatim
```

So every install is one move: call `create_artifact` pointing at the staged page. **This skill uses
no shell.** The only tools it calls are `Glob`, `Read`, `Write`, `list_artifacts`, `create_artifact`
and the MCP probes in Step C5. Do not call `mcp__workspace__bash`: Claude Desktop denies it in
Cowork sessions on current builds, and nothing here needs it. Never retype or reconstruct a page,
never point `html_path` at the plugin or skills directories, and never run `create_artifact` calls
in parallel — install one dashboard at a time, verifying as you go.

**Caching contract:** Cowork only caches a dashboard's MCP tool results when the gateway tool
advertises `annotations.readOnlyHint: true`. Every read-only tool a dashboard calls must carry that
annotation in its server's tool catalog — without it, every re-render refetches and rendering becomes
racy. Every tool the workspace dashboards call carries it.

## Step C1 — Read the bundle the bridge staged

Locate the manifest with `Glob` for `**/systemprompt/artifacts/manifest.json` across the connected
folders, then `Read` it. It is small: one record per dashboard —
`{ "id", "name", "description", "version", "isStarred", "mcpTools": [...], "plugins": [...] }` —
and the pages are not in it. Do not read the `.html` files into context: `create_artifact` copies a
page from its path, so reading one is wasted context and a sign you are about to retype what should
have been copied.

Keep only the records whose `plugins` does **not** name `systemprompt-admin`. That set **is** the
user bundle: count it, never assume it. On this instance expect `todo-bulletin`, `upcoming-deals`,
`pipeline-open-deals` and `recent-activity`. The admin records in the same manifest belong to
`systemprompt_setup_admin`.

If the manifest is absent, the bridge has not synced since it was installed, or the workspace
folder is not connected to this session. Say exactly that, tell the user to press **Sync** in the
bridge (and to connect the `Systemprompt` folder if the session shows none), and stop. Do not look
for a script, do not copy pages by hand, and do not write a zero-install receipt.

## Step C2 — Diff bundled against installed

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

## Step C3 — Install what is missing

For each **missing** record, call the built-in `create_artifact` tool — sequentially, never in
parallel — with:

- `id` and `description` from the record,
- `html_path` set to the page beside the manifest, exactly as `Glob` spelled the directory:
  `<workspace>/systemprompt/artifacts/<id>.html`. That folder is connected, so the path is accepted
  as it is,
- `mcp_tools` set to the record's `mcpTools`, verbatim — without it the dashboard cannot call its
  MCP server and will never load data,
- `name` and `starred` from the record if the tool's schema exposes such fields; if it does not,
  skip them silently.

**Verify** with one `list_artifacts` after the whole batch: every bundled id must appear, **and for
each one the installed record must carry the same tool allowlist as its manifest record** — compare
the listed `mcp_tools` (however the library names the field) against the record's `mcpTools` for
that id, verbatim. A dashboard installed with another dashboard's allowlist renders but every data
fetch fails with "not in this artifact's mcp_tools allowlist", so a mismatch is a failed install:
delete that one artifact, re-run its `create_artifact`, and re-verify. An artifact counts as
installed only when it appears in the list with the right allowlist — never because the create call
"should have" worked. If any create call errored, fix and retry that one before the final listing.

**Never diagnose a tool failure as a role, permission or governance problem.** The governance chain
evaluates *before* the tool runs and returns an explicit verdict naming the policy — a denial
reaches you as a refusal or an approval prompt, never as a generic tool failure. This skill is in
your manifest only because the grant already passed.

For each **stale** record, tell the user it is out of date and offer to replace it. Do not silently
overwrite an artifact the user may have edited.

If one artifact genuinely fails after a retry, record it under `failed` and carry on with the rest —
but a workspace-path rejection means `html_path` did not point at the staged page: re-read the path
from `Glob` and retry.

## Step C4 — Write a receipt

Each entry in `created` and `alreadyPresent` is an object, not a bare id — capture whatever
identifying reference `create_artifact`'s response and the `list_artifacts` verification entries
actually expose for that record (an id, a url, however the library names it — same caution as the
`mcp_tools` field in Step C3: read what the tool gives back, do not assume a field name). At minimum
carry `id` and `name`; add `ref` when the tool exposes something beyond the id. This is what lets
Step C6 point the user at each dashboard by name instead of just a count.

`Write` the receipt to `outputs/setup-receipt.json` in the session's outputs directory, with the
current UTC time in `checkedAt`:

```
{ "checkedAt": "<ISO-8601 UTC>", "plugins": ["..."], "bundled": N, "installed": N,
  "created": [{ "id": "...", "name": "...", "ref": "..." }],
  "alreadyPresent": [{ "id": "...", "name": "...", "ref": "..." }],
  "stale": ["..."], "superseded": ["..."], "failed": [] }
```

`plugins` is the distinct `plugins` values across the installed records; `bundled` is the record
count from Step C1 — count them, never assume. Never write into the plugin or skills directories,
or into the bundle folder: they are replaced wholesale on every sync. `installed` must be the count
confirmed by the final library listing, nothing else. If the write fails, do not fail the run:
report the same receipt inline in Step C6 so the result is still visible.

## Step C5 — Check the connections the dashboards need

Collect the distinct MCP servers named across the installed records' `mcpTools` (the part between
`mcp__` and the next `__`) and run one small read-only probe per server, executed as the signed-in
user:

| server | probe |
|--------|-------|
| `odoo` | `crm_lead_search` with `{ "limit": 1 }` — one probe covers all four workspace dashboards; they share the server |

Call each probe by its **full wire name**, `mcp__<server-id>__<tool>`. The server segment is the id
exactly as `services/mcp/*.yaml` spells it, **hyphens and all**. Nothing normalises a hyphen to an
underscore, so an underscore where the id has a hyphen is not a near miss that still resolves; it is
"No such tool available". Read the name off the installed record's `mcpTools`, which already carries
the correct string, rather than retyping it.

Pass the arguments in the table verbatim. These tools reject unknown keys, so a plausible-looking
extra fails the probe and reads as a broken server.

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

## Step C6 — Report honestly

Never report a number you did not verify, and report a partial result as partial. Name the plugins
the dashboards came from. Then list what was created and what was already there **by name, each with
its Step C4 reference** — not just a count — so the user can jump straight to a dashboard instead of
hunting the Library for it; fall back to naming it plainly if no `ref` was available for that one.
Then note anything stale, superseded, or failed, and which servers answered. If everything was
already present, say so — that is a successful run, not a no-op.

If Step 1 showed you hold `systemprompt_setup_admin`, run it now for the control-plane dashboards
and the admin CLI check.

---

# Codex setup

Get OpenAI Codex CLI routing its inference through the systemprompt.io gateway, so every request is
authenticated, audited, and attributed like any other governed agent. The heavy lifting is done by
the desktop bridge — this walks the user through it and verifies the result.

## Step X1 — Install and sign in to the bridge

The bridge manages host-app profiles; Codex CLI is one of its supported hosts. If it is not
installed yet, download the installer for the user's OS. The asset names are version-less on
purpose, so these stay permanent links:

| OS | Asset |
|----|-------|
| macOS (Apple Silicon **and** Intel — one universal build) | `systemprompt-internal-bridge-macos.dmg` |
| Linux x86_64 | `systemprompt-internal-bridge-linux-x86_64.tar.gz` |
| Linux aarch64 | `systemprompt-internal-bridge-linux-aarch64.tar.gz` |
| Windows x86_64 | `systemprompt-internal-bridge-windows.exe` |

```bash
# Every merge to main publishes bridge-v<version> beside the gateway image, so
# `latest` is the build released with the deployed gateway; the admin Bridge
# Setup page links the exact version.
curl -LO https://github.com/systempromptio/systemprompt-internal/releases/latest/download/systemprompt-internal-bridge-macos.dmg
```

On macOS, open the dmg and drag **Systemprompt Internal
Bridge.app** to `/Applications`; the full walkthrough is `docs/install/bridge-macos.md`.

Then have them sign in with their systemprompt account (Odoo credentials or passkey). Signing in
links the device through `/bridge-auth/device-link` automatically.

## Step X2 — Enable the Codex CLI agent

In the bridge's agents step (or Settings → Agents), enable **Codex CLI**. The bridge writes the
managed profile itself — the model provider in `~/.codex/config.toml` pointing at the gateway and
the credential to authenticate with. Never hand-edit that file to point at the gateway; the bridge
re-syncs managed keys and will flag manual drift as stale.

## Step X3 — Verify a governed request

Have the user run any small Codex CLI prompt, then confirm the request landed in the audit spine:

```bash
systemprompt infra logs request list --limit 5
```

A row with the user's identity appearing right after the Codex run means the wiring is complete.
No row means Codex is still talking to OpenAI directly — re-check Step X2 (the bridge's Codex card
should read "Installed", not "stale" or "unmanaged"). To read back what that request cost, use
`governance_readback`.

## What Codex does not get — dashboards

Codex gets **no dashboards**, and this is not a gap to work around. Codex has no artifact library:
no `create_artifact`, no `list_artifacts`, no persistent gallery, and no CLI equivalent. Its one
HTML surface, the inline visualization, renders into a thread-scoped scratch directory and is
explicitly blocked from calling tools (`callMcp` rejects with "Inline visualizations cannot call
tools", and the page's CSP sets `connect-src 'none'`), so a dashboard rendered there could never
fetch Odoo data. Say the dashboards live in Claude Cowork and are installed there by this same
skill — plus `systemprompt_setup_admin` for the control-plane set, which only admins hold.

## What Codex does not get — Odoo identity

Odoo access from Codex flows through the gateway's MCP surface and the user's linked Odoo identity;
if Odoo tools fail with a missing-identity error, the user connects Odoo on their profile page
(`/admin/profile`).
