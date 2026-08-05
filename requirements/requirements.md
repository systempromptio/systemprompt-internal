# Astound Digital × systemprompt — POC Requirements

**Status:** Agreed in principle (Ed Burton / Viktor Durnev call, with prior alignment from Roman). See [architecture.md](architecture.md) for how these requirements will be delivered.

## Executive summary

Astound Digital develops on client projects using a centrally maintained set of AI skills and rules, currently managed in Cursor via a shared repository that developers branch from. The agreed proof of concept moves that management into a governed gateway: a developer assigned to a project receives only an API key and a tenant ID, runs near-vanilla Claude Code, and the full harness — skills, agents, MCP servers, governance rules — is delivered and enforced centrally. A skill updated once at the gateway propagates to every developer assigned to that profile.

The POC is scoped to development tooling for Salesforce work (Commerce Cloud / Storefront Next), evaluated by a one-person pilot. Success means Astound's existing skill set works through the managed flow end-to-end, with governance, auditing, and per-client profile control demonstrated. Customer-facing delivery (e.g. offering skills to client QA teams as a product) is explicitly a later phase.

## Agreed deliverables

1. **Astound foundation template repo** — already shared (Viktor has admin access). Serves as the starting point for every project: each engagement forks it, syncs common organisation-wide skills from upstream, and adds project-specific customisation.
2. **Central skill and harness management per tenant/profile** — developers are onboarded with a key + tenant ID; the gateway serves the complete skill set. Updates are made once, centrally, and reach all assigned developers.
3. **Migration of Astound's Cursor skill set** into a dedicated development area of the template, mirroring the existing categories:
   - **plan** — open-spec / architecture design rules
   - **build** — Storefront Next development (React Router patterns, libraries)
   - **release** — commit messages, pull requests
   - **test** — verification via Playwright before declaring work done
   *(Dependent on approval to share the skills — see Open items.)*
4. **RAG / knowledge-bank integration** — Astound's "project context" system (workshop transcripts, Jira tickets, Confluence pages), already exposed as an MCP server, connected through the platform so agents use it automatically. Project-scoped for the POC; intended as the foundation for an organisation-wide knowledge bank.
5. **Salesforce integration** — Salesforce MCP already built into the template, covering both Salesforce Core (configuration-led) and Commerce Cloud (code-led) contexts.
6. **One-command isolated developer setup** — installing a project environment spins up a sandboxed (Docker) Claude Code with all tenant skills and connections, without touching the developer's global configuration.

## Out of scope for the POC (future phases)

- **QA-automation product** — packaging a Playwright test-authoring skill for client QA teams (manual → automated QA), offered through the gateway with a restricted skill view.
- **Customer-facing delivery** — the same skills surfaced to end clients via Claude Cowork or similar, beyond developer tooling.
- **Cursor glob-rule emulation** — path-scoped rule application (Cursor's globs feature) enforced at the gateway; noted as feasible, deferred until a concrete need appears.

## Open items / dependencies

- **Approval to share Astound's skill set** with systemprompt so it can be pre-embedded in the template (owner: Roman).
- **Access to the project-context RAG MCP server** so it can be connected out of the box (owner: Roman/Viktor).
- **Meeting-notes upload flow** — transcripts are uploaded manually and selectively; requires a decision on who may upload, what they may upload, and whether the knowledge bank is project-scoped, organisation-scoped, or both (agreed: eventually both).
- **Claude Code approval at Astound** — Cursor is currently the only approved development tool; migration to Claude Code is under consideration and is a precondition for the gateway approach.
