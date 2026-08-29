# Inspect

The per-thing microscope for the control plane. Three subjects, one skill: a **request**, a
**conversation**, and the **MCP surface** underneath both. For fleet-wide rollups and the standing
daily report, use `report` instead — this skill is always about one specific thing.

## Ask me things like

- "Why was this request denied / what did it cost?"
- "Show me the data trail behind this conversation."
- "Is the systemprompt MCP server healthy, and what tools does it expose?"

## How commands run

Everything below runs through the admin `systemprompt` MCP server, which exposes exactly one tool —
also named `systemprompt` — taking a single `command` argument. Pass the CLI command **without** the
`systemprompt` prefix:

```json
{ "command": "infra logs request list --limit 20" }
```

The server is admin-only. A non-admin caller gets
`Insufficient permissions. User must have one of: ["admin"]` — that is the gate working, not a bug.

---

# 1. One request

Follow an inference request through the gateway: from the client's `/v1/messages` call, through model
routing and the governance pipeline, to the provider and back.

Clients call `/v1/messages`. The gateway routes by model pattern to a provider (the `gateway.routes`
block in the active profile — `claude-*` → anthropic, `gpt-*` → openai), runs the synchronous
governance pipeline, forwards to the provider, and audits the result. Every hit is one row carrying
`user_id`, `tenant_id`, `session_id`, and `trace_id`, so a single id reconstructs the whole chain.

### List recent gateway requests

```bash
systemprompt infra logs request list --limit 20
systemprompt infra logs request list --since 1h --model claude
systemprompt infra logs request list --since 24h --provider anthropic
```

Each row is one `/v1/messages` hit: provider, model, token counts, cost, latency, status. Filters are
`--since`, `--model`, `--provider`, `--limit`. There is **no `--status`** here — status filtering
lives on `trace list`.

### Reconstruct one request

`audit` accepts an AI request id, a task id, or a trace id and rebuilds identity → policy evaluations
→ prompt → response → tool calls → cost:

```bash
systemprompt infra logs audit <request-id> --full
systemprompt infra logs audit <request-id> --json     # machine-readable
systemprompt infra logs request show <request-id> --full   # lighter, one request id
```

### Follow the tool calls it spawned

```bash
systemprompt infra logs trace show <trace-id> --all        # steps, AI requests, MCP calls, artifacts
systemprompt infra logs trace list --status failed --limit 10
systemprompt infra logs trace list --has-mcp --tool <tool-name>
systemprompt infra logs tools list --limit 20              # raw MCP tool executions
```

### Watch live while reproducing

```bash
systemprompt infra logs stream --since 30s                 # tail -f (alias: follow)
systemprompt infra logs view --level error --since 1h
```

**Workflow:** `request list --since 1h` → `audit <id> --full` → `trace show <trace-id> --all` →
`plugins mcp logs <server>` if a tool failed.

---

# 2. One conversation

Show the structured data behind a conversation happening right now: every message, AI request, tool
call, governance decision, and cost, keyed to one session. This is the self-referential demo — point
it at the active session and it returns the whole trail.

## The id chain

```
session_id  ->  context_id  ->  task_id  ->  trace_id  ->  request_id
(gateway)       (conversation)  (a turn)     (execution)   (one /v1/messages hit)
```

No single command returns all of it; compose the steps, and use `infra db query` (read-only SQL)
where there is no dedicated command for a link.

**Pick the right anchor.** Gateway-client conversations (any Anthropic-SDK client hitting
`/v1/messages`) are keyed on `session_id`. A2A agent runs are **not** — they group by `context_id`,
with `agent_tasks` as the bridge. This instance ships no A2A agents (see section 3), so in practice
you will anchor on `session_id`; the `context_id` path below is there for when agents return.

### Find and open the conversation

```bash
systemprompt analytics sessions live                  # active sessions: session_id, request_count, last activity
systemprompt analytics conversations list --limit 5   # context_id, name, message/task counts
systemprompt core contexts show <context_id>          # name, task count, message count, timestamps
```

### Pull its requests, tools, rulings, and cost

```bash
systemprompt infra db query "SELECT id, model, status, input_tokens, output_tokens, cost_microdollars, created_at FROM ai_requests WHERE session_id = '<session_id>' ORDER BY created_at DESC"
systemprompt infra logs audit <request_id> --full
systemprompt infra logs trace show <trace_id> --all
systemprompt infra db query "SELECT decision, tool_name, policy, reason, created_at FROM governance_decisions WHERE session_id = '<session_id>' ORDER BY created_at DESC"
systemprompt analytics costs summary --since 24h
```

### If agents are ever reintroduced: the `context_id` path

Anchor on `context_id` and use `agent_tasks` as the bridge — one row per turn, carrying `task_id`,
`session_id`, `trace_id`, `agent_name`, `user_id`, and the `started_at`/`completed_at` window:

```bash
systemprompt infra db query "SELECT task_id, agent_name, user_id, session_id, trace_id, started_at, completed_at FROM agent_tasks WHERE context_id = '<context_id>' ORDER BY started_at"
```

An agent's own tool calls are governed under a **separate MCP session**, so its `governance_decisions`
rows are not keyed by the conversation `session_id`; join by `user_id` within the turn's time window.
(`governance_decisions` carries no `context_id`/`task_id` column, so that join is a time window rather
than an exact key — a known core limitation, not a data fault.)

---

# 3. The MCP surface and skills catalogue

### Servers and tools

```bash
systemprompt plugins mcp list                           # configured MCP servers
systemprompt plugins mcp status                         # runtime status: running, PID, port
systemprompt plugins mcp validate                       # validate server configurations
systemprompt plugins mcp tools                          # all tools across servers
systemprompt plugins mcp tools --server systemprompt    # the admin server only
systemprompt plugins mcp logs systemprompt              # server logs for debugging
```

> **Enumerate tools with the direct CLI, never through the passthrough.** `plugins mcp tools` needs
> the admin MCP-server session. Run through the *direct* CLI (an authenticated admin) it returns the
> tool list. Run *through* the `systemprompt` MCP passthrough, the nested CLI has no admin MCP session
> and the enumeration comes back empty with `auth_required` — the nested process cannot authenticate
> to the live server. Known passthrough limitation, tracked as core tech debt.

### Calling the systemprompt tool directly

```bash
systemprompt plugins mcp call systemprompt systemprompt --args '{"command":"core skills list"}'
systemprompt plugins mcp call systemprompt systemprompt --args '{"command":"infra services status"}'
```

Admin-only: namespaced `mcp__systemprompt__*`, and the governance `scope_check` policy denies it
unless the caller has `admin` scope. If `plugins mcp status` shows `systemprompt` running and a
`call` returns CLI output rather than an auth error, the admin is authenticated end to end.

### Skills catalogue

```bash
systemprompt core skills list                           # configured skills
systemprompt core skills show <skill_id>                # config + instruction body for one skill
```

Skills live on disk under `services/skills/<id>/` (a `config.yaml` plus a body file) and reach a
client through the plugin that includes them. The YAML is bootstrap state: the stack ingests it at
startup and the database owns it at runtime, so after editing skill YAML you must restart:

```bash
systemprompt infra services restart api
```

### Agents

**This instance ships no A2A agents, deliberately** — `services/config/config.yaml` says so, nothing
lives under `services/agents/`, and no plugin bundles one. `admin agents list` returning nothing is
the correct answer here, not a fault; report it as such. Skills, MCP servers and artifacts carry the
capability instead. If agents are ever added, `admin agents {list,show,validate,tools,message,task,logs}`
is the surface, and `infra logs trace list --agent <name>` follows what they ran.
