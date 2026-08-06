# systemprompt.io × systemprompt — POC Architecture

**Status:** Agreed approach for delivering the [requirements](requirements.md).

## Executive summary

The architecture is three layers: the **systemprompt core** (the governed gateway and platform, consumed as a library), the **Systemprompt Internal foundation template** (this repository — Systemprompt Internal's organisation-wide configuration), and **per-project forks** of that template (one per client engagement). Developers connect near-vanilla Claude Code through the gateway; everything Systemprompt Internal-specific — skills, agents, MCP servers, governance policy — lives in configuration served per tenant, so central updates reach every developer without touching their machines.

## 1. Layering

| Layer | What it is | Who changes it |
|-------|------------|----------------|
| systemprompt core | Gateway, governance spine, tenant profiles, admin console, bridge. The stable foundation worth evaluating deeply. | systemprompt |
| Systemprompt Internal foundation template (this repo) | Organisation-wide skills, agents, MCP wiring, plugins, branding. Deliberately fast to iterate. | Systemprompt Internal platform owners (Viktor) |
| Per-project forks | One fork per client engagement; syncs common skills from the template upstream, adds project-specific skills and data sources. | Project teams |

Templates are intentionally lightweight on top of core: core provides the durable infrastructure, templates let each engagement be stood up and customised rapidly.

## 2. Developer flow

1. Developer is assigned to a project and given an API key + tenant ID.
2. They sign in (corporate email; Odoo SSO available) and run the one-line install.
3. The install creates an **isolated Docker sandbox** running Claude Code, pre-configured with the tenant's full harness — skills, MCP servers, hooks, governance — leaving the developer's global setup untouched.
4. From then on, skill and rule updates made centrally at the gateway appear automatically; nothing to pull or reconfigure.

## 3. Governance

Every inference request and every MCP tool call passes through the gateway's synchronous pipeline — **scope check → secret scan → blocklist → rate limit** — and every decision is audited with a trace linking identity → agent → tool → result → cost. This gives Systemprompt Internal:

- per-client control over which skills and tools a profile can see and use;
- role-based access control on MCP servers (e.g. who may upload meeting transcripts to the knowledge bank, who may only read);
- full audit and cost attribution per developer, per project, per client;
- parity with (and extensibility beyond) Cursor's built-in governance — gaps such as path-scoped rules can be implemented in the gateway if needed.

## 4. Skill and marketplace distribution

Skills, agents, and MCP servers are declared as flat configuration and aggregated into **plugins** by reference. A tenant profile selects which plugins a user sees, so:

- Systemprompt Internal developers on a delivery project get the full dev skill set (plan / build / release / test);
- a future client QA user would see only the QA skill — same platform, restricted view;
- validation at load time guarantees every referenced skill and server actually exists.

## 5. Integrations

- **Odoo MCP** — server-to-server auth into Odoo; covers CRM (`crm.lead`), messaging (`mail.message`), and activities (`mail.activity`). Already in the template.
- **Systemprompt Internal knowledge-bank / RAG MCP** — Viktor's existing project-context MCP (transcripts, Jira, Confluence) connected through the gateway with RBAC on upload/read. To be onboarded once access is granted.
- **systemprompt admin MCP** — admin tooling exposed to platform owners, so governance itself can be managed and inspected by agents.

## 6. Pilot deployment shape

- Deployed on Systemprompt Internal-controlled infrastructure (demoed on Fly; any Docker-capable host works), with an isolated Postgres per tenant.
- Admin console available for infrastructure and governance assessment (Vlad: infrastructure, Roman: admin/governance, Viktor: integration and skills flow).
- The foundation template is pre-loaded with Systemprompt Internal's skills, the Odoo MCP, and — once access is granted — the RAG MCP, so the pilot evaluation is "install and use", not "migrate and build".
