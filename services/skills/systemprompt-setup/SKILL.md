# Systemprompt Setup

The front door for setting up systemprompt.io in whatever host you are running in. This skill does
not do the setup itself — it works out where you are and hands over to the matching host-specific
skill, so users only ever have to remember one name.

## Ask me things like

- "Set up systemprompt."
- "Set up my workspace."
- "Get me connected to the CRM."
- "Is my systemprompt setup complete?"

## How to route

Decide which host you are running in, then load and follow the matching skill:

| You are running in | Signs | Use |
|--------------------|-------|-----|
| Claude Cowork | A session VM under `/sessions/`, a `create_artifact` tool that takes `html_path`, an `outputs/` directory | `systemprompt-setup-cowork` |
| Codex CLI | OpenAI Codex CLI environment, no Cowork artifact tools | `systemprompt-setup-codex` |

If neither matches (plain Claude Code, another MCP client), there is nothing host-specific to
install: verify the Odoo MCP connection works by listing the available Odoo tools, and point the
user at their profile page (`/admin/profile`) to connect Odoo if tools fail with a missing-identity
error.

**Admin users, in Cowork only:** there is a second set of dashboards — users, activity, usage —
installed by `admin_workspace_setup_cowork` from the admin plugin. It is separate from the CRM
dashboards and needs the admin role. Mention it after the CRM setup finishes; it is subject to the
same host rule below.

## Dashboards are a Cowork feature — everywhere else, say so

Never attempt an artifact installation outside Cowork. `create_artifact` with an `html_path` only
exists there, and every past failure came from guessing at it elsewhere. In particular, **Codex CLI
has no artifact library**: no `create_artifact`, no `list_artifacts`, no persistent dashboard
gallery, and its inline visualizations are blocked from calling MCP tools, so a page rendered there
could never load Odoo data. There is no CLI substitute either — `coworkctl` does not exist.

Outside Cowork the correct outcome is to tell the user dashboards do not apply on this host and
finish the host's own setup, not to stage HTML files or write a receipt reporting zero installs.
