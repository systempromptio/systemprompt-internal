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

Never attempt the Cowork artifact installation outside Cowork — `create_artifact` with an
`html_path` only exists there, and every past failure came from guessing at it elsewhere.
