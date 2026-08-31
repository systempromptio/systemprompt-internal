# Systemprompt Setup

The front door for setting up systemprompt.io in whatever host you are running in. This skill does
not do the setup itself — it works out who you are and where you are, then hands over to the
matching skill, so users only ever have to remember one name.

## Ask me things like

- "Set up systemprompt."
- "Set up my workspace."
- "Get me connected to the CRM."
- "Is my systemprompt setup complete?"

## Step 1 — Establish what this account can reach

There is no identity tool on this instance: nothing reports back who the account is or what it was
granted. Establish the two things setup actually depends on, from what you can see:

- **Which skills you were handed.** Your own skill list *is* the grant — the bridge syncs exactly
  what the signed manifest allowed. If `systemprompt_setup_admin` is among them, this is an admin
  account; if it is not, it is a user account, and that skill is not missing.
- **Whether Odoo is linked.** Call `crm_lead_search` with `{ "limit": 1 }`. A result means linked; an
  authentication or missing-identity error means it is not — say so and point at `/admin/profile` to
  add an Odoo login and API key. Carry on routing either way.

State both back in one short sentence before you route, so the user knows which path they are on.

## Step 2 — Route on host

| You are running in | Signs | Use |
|--------------------|-------|-----|
| Claude Cowork | A session VM under `/sessions/`, a `create_artifact` tool that takes `html_path`, an `outputs/` directory | `systemprompt_setup_cowork` |
| Codex CLI | OpenAI Codex CLI environment, no Cowork artifact tools | `systemprompt_setup_codex` |

If neither matches (plain Claude Code, another MCP client), there is nothing host-specific to
install: the Odoo check from Step 1 is the whole of setup. Say so and stop.

## Step 3 — Route on role

Setup is split by role, and the split is enforced by the grant, not by this skill: an admin holds
`systemprompt_setup_admin`, a user does not, and nothing you do here can change that.

- **`systemprompt_setup_admin` is in your skill list**, and you are in Cowork: after
  `systemprompt_setup_cowork` finishes the workspace dashboards, run `systemprompt_setup_admin` for
  the control-plane dashboards (users, activity, usage) and the admin CLI check. Two skills, run in
  that order — the workspace ones are the ones an admin uses daily too.
- **It is not in your skill list**: the host skill is the whole of setup. `systemprompt_setup_admin`
  is not in your grant, is not missing, and must not be mentioned — there is nothing for you there.

Never infer a role from an email address or a username. The presence of the admin setup skill in
your own list is the only source, because that list is what the manifest actually granted.

## Dashboards are a Cowork feature — everywhere else, say so

Never attempt an artifact installation outside Cowork. `create_artifact` with an `html_path` only
exists there, and every past failure came from guessing at it elsewhere. In particular, **Codex CLI
has no artifact library**: no `create_artifact`, no `list_artifacts`, no persistent dashboard
gallery, and its inline visualizations are blocked from calling MCP tools, so a page rendered there
could never load Odoo data. There is no CLI substitute either — `coworkctl` does not exist.

Outside Cowork the correct outcome is to tell the user dashboards do not apply on this host and
finish the host's own setup, not to stage HTML files or write a receipt reporting zero installs.
