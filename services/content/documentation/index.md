---
title: "Get Started with Systemprompt Internal"
description: "Start here. Two paths: standing the platform up for your organization, or running it locally as a developer. Pick the one that describes you."
author: "systemprompt.io"
slug: ""
keywords: "get started, how to use, getting started, odoo ai, desktop bridge, admin setup, dashboard, authentication"
kind: "guide"
public: true
tags: ["documentation", "getting-started"]
published_at: "2026-02-18"
updated_at: "2026-08-06"
after_reading_this:
  - "Know which of the two setup paths applies to you"
  - "Get connected and ask your first question"
  - "Know which skills to reach for and how to ask"
  - "Find the reference documentation for each step"
---

# Get Started

Systemprompt Internal puts your business inside Claude. You ask in plain
English. It answers from live Odoo data, confirms with you before it changes
anything, and records everything it does. Odoo stays the system of record; this
is the AI and communication layer on top of it, and you host all of it.

There are two ways to arrive here. Pick yours.

## I need to set this up for my organization

**[Standing Up the Gateway →](/documentation/use-case-admin)**

You own the Odoo instance and the deployment. You will point the deployment at
your Odoo database, provision users, and hand the bridge to your team.

*Prerequisites: Odoo admin access and shell access to the deployment. About an
hour, once per organization.*

## I am a developer running this locally

**[Connect Claude Code →](/documentation/connect-claude-code)**

Clone, build, start the gateway, register at `/admin/login`, then one command
with the one-shot code from your profile page. Includes the clean-state
verification procedure for the connect path.

*Prerequisites: Docker, just, a Rust toolchain, and one provider API key.*

## The Short Version

For a user, getting connected is three steps:

1. **Install the desktop bridge.** Downloads are on the [homepage](/). It runs
   in your menu bar or system tray and keeps your skills in sync inside Claude
   Code, Cowork, and Codex.
2. **Enrol a passkey.** There is no signup form and no password. An operator
   creates your account from the CLI and sends you a one-shot setup link; you
   open it and create a passkey. See
   [Authentication](/documentation/authentication).
3. **Link your Odoo account and ask.** Generate an API key in Odoo under
   Preferences → Account Security, link it on `/admin/profile`, then try *"Give
   me a full briefing on my biggest account"* or *"What is in my pipeline this
   quarter?"* The skills are already installed. Every call runs as you in Odoo,
   under your own record rules.

## What to Ask

Browse the [full skills catalogue](/skills/) — every skill lists what it does and
example questions covering pipeline, leads, partners, activities, notes,
consultancy, brand, and governance.

## Reference

**Running the platform:**

- [Standing Up the Gateway](/documentation/use-case-admin) — the operator journey end to end
- [Connect Claude Code](/documentation/connect-claude-code) — the local developer path
- [Authentication](/documentation/authentication) — passkeys, Odoo identity linking, sessions and route protection
- [Connect Odoo](/documentation/odoo) — instance requirements, server config, per-user API keys
- [Dashboard Usage](/documentation/dashboard) — real-time metrics, activity feed, and health indicators
- [Gateway API](/documentation/gateway-api) — the `/v1/messages` endpoint and its governance

## Under the Hood

Step-by-step walkthroughs of the governance and tracing machinery, for technical
readers:

- [Setup & Authentication](/documentation/demo-terminal-setup) — bring the platform up and authenticate
- [Governance Decisions](/documentation/demo-terminal-agents) — agents making governed tool calls
- [Audit Trails & Costs](/documentation/demo-terminal-audit) — inspect audit logs and cost attribution
- [Governance API](/documentation/demo-terminal-governance) — drive policy decisions from the CLI
- [MCP Access Tracking](/documentation/demo-terminal-mcp) — watch MCP tool access in real time
- [Request Tracing & Benchmark](/documentation/demo-terminal-tracing) — trace requests end-to-end
- [Agent Tracing](/documentation/demo-terminal-agent-tracing) — follow a single agent's lifecycle
- [Detailed Breakdown](/documentation/demo-breakdown) — what happens under the hood on a single call
- [An Allowed Call](/documentation/demo-happy-path) — a skill that passes governance
- [A Refused Call](/documentation/demo-refused-path) — secret detection denying a tool call
