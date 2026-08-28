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
| Claude Cowork | A session VM under `/sessions/`, a `create_artifact` tool that takes `html_path`, an `outputs/` directory | `systemprompt_setup_cowork` |
| Codex CLI | OpenAI Codex CLI environment, no Cowork artifact tools | `systemprompt_setup_codex` |

If neither matches (plain Claude Code, another MCP client), there is nothing host-specific to
install: call `comms_whoami` with `{}` — it reports who the user is, their roles, whether Odoo is
linked, and which plugins, servers and skills they were granted — and point the user at their
profile page (`/admin/profile`) to connect Odoo if it reports the link missing.

**One setup, every role.** There is no separate admin setup. In Cowork the setup skill installs the
dashboards of every plugin bundle the bridge mounted, and the bridge mounts exactly what the user's
signed manifest granted — so an admin simply ends up with the control-plane dashboards (users,
activity, usage) alongside the workspace ones, and a salesperson does not. Never tell an admin to
look for a second setup skill; run the same one.

## Dashboards are a Cowork feature — everywhere else, say so

Never attempt an artifact installation outside Cowork. `create_artifact` with an `html_path` only
exists there, and every past failure came from guessing at it elsewhere. In particular, **Codex CLI
has no artifact library**: no `create_artifact`, no `list_artifacts`, no persistent dashboard
gallery, and its inline visualizations are blocked from calling MCP tools, so a page rendered there
could never load Odoo data. There is no CLI substitute either — `coworkctl` does not exist.

Outside Cowork the correct outcome is to tell the user dashboards do not apply on this host and
finish the host's own setup, not to stage HTML files or write a receipt reporting zero installs.
