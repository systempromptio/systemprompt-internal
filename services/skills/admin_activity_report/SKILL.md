# Activity Report

Answer "what has been happening on this platform?" - who is live right now, which requests ran, which tool calls
failed, and where the money went. This is the recent-activity report; for the roster of people use
`admin_user_report`, for fleet-wide rollups use `analytics_dashboards`, and for one request in full use
`inspect_ai_requests`.

## Ask me things like

- "What's happened in the last hour?"
- "Show me recent usage and activity."
- "Who is on the system right now?"
- "What failed today?"
- "Where is our spend going this week?"

## When to Use

Use this skill when the question is about **events over a time window** - requests, sessions, tool calls,
failures, spend. It is the daily standing report for the control plane. It deliberately does not duplicate
`analytics_dashboards`; when the answer needs period-over-period trends or per-agent/per-tool rollups, say so
and switch to that skill.

## How commands run

Every command runs through the admin `systemprompt` MCP server's single `systemprompt` tool, which takes one
`command` argument - the CLI command **without** the `systemprompt` prefix:

```json
{ "command": "infra logs request list --limit 50 --json" }
```

Add `--json` when the result feeds a dashboard artifact; the CLI then emits a typed envelope
(`{"artifact_type":"table","columns":[...],"items":[...]}`, or `presentation_card` with `sections` for stats,
or `message` when there is nothing to report). Omit `--json` when answering in chat, and summarise rather than
pasting raw output.

## Time windows

`--since` accepts `30s`, `1h`, `24h`, `7d`, `30d` and defaults to `24h` on the analytics commands. Always state
the window you used in the answer - "9 requests in the last hour" and "9 requests in the last 30 days" are very
different findings.

## Live right now

```bash
systemprompt analytics sessions live                  # sessions active this moment
systemprompt analytics sessions stats --since 24h
systemprompt analytics conversations list --limit 20  # recent conversations: context_id, name, counts
```

## Recent requests

```bash
systemprompt infra logs request list --limit 50       # one row per /v1/messages hit
systemprompt infra logs request list --since 1h --provider anthropic
systemprompt infra logs request stats
```

Columns are `request_id`, `timestamp`, `provider`, `model`, `tokens`, `cost`, `latency_ms`, `status`. There is
no `--status` filter here - status filtering lives on `trace list`. To explain a single row, hand the
`request_id` to `inspect_ai_requests`.

## What failed

```bash
systemprompt infra logs view --level error --since 1h
systemprompt infra logs trace list --status failed --limit 20
systemprompt infra logs trace list --agent <agent-name> --status failed
systemprompt infra logs trace show <trace-id>
```

`trace list` covers MCP tool calls (PreToolUse → decision → spawn → result); `request list` covers gateway
inference. They are different paths through the same audit spine - check both before concluding "nothing
failed".

## Where the spend went

```bash
systemprompt analytics costs summary --since 7d
systemprompt analytics costs breakdown --by model      # or: --by provider, --by agent
systemprompt analytics requests models                 # routing mix
```

**Read attribution before reporting agent numbers.** Gateway inference (Cowork, any Anthropic-SDK client, the
playground) lands in `ai_requests` but carries no agent task, so it shows as `unattributed` under
`costs breakdown --by agent` and produces no rows in `analytics agents list`. That is the expected shape of a
gateway-driven workspace, **not** a tracking failure - do not report it as one. Say "spend is gateway
inference, not attributed to named agents" and pivot to `--by model` / `--by provider`.

## Typical workflow

1. `analytics sessions live` - is anyone on right now?
2. `infra logs request list --since 24h` - the recent traffic, with model, cost, latency, status.
3. `infra logs trace list --status failed` + `infra logs view --level error --since 1h` - what broke.
4. `analytics costs breakdown --by model` - where the money went (mind the attribution note above).
5. Drill down with `inspect_ai_requests` on a specific `request_id`, or `inspect_conversation` on a session.
6. Close with a short verdict: volume, spend, error rate, and the one thing worth acting on.
