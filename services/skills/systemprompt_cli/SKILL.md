---
name: systemprompt-cli
description: Operate and inspect the Systemprompt Internal platform through the admin systemprompt MCP server and CLI - is the instance healthy, services status, which MCP servers are configured or running, recent errors, list skills, plugins or users, who is using AI, or run any CLI command.
---

# systemprompt CLI Reference

## Typed tools first

The `systemprompt` MCP server (admin-only) has typed tools for the common questions. Prefer them to a hand-built CLI command.

| Question | Tool |
|----------|------|
| What did one person do: conversations, active days, skills, titles | `user_activity` |
| One row per conversation in a window | `conversation_list` |
| Who is spending, how much | `usage_by_user` |
| Debug one request (not for counting activity) | `request_log` |
| What was said in one request | `conversation_audit` (`max_chars` 300-500) |
| Who is registered, with which role | `users` |
| The dashboard | `admin_report` (`{"report":"costs","days":7}`) |

For people and usage questions use the skills `admin_person_activity` (one person) and `admin_ai_usage` (spend, adoption). Rows are named `<user_id> (<display name>)`; the id is what other tools take as `user`. Map ids to emails with `users`.

**Paging.** `limit` is clamped to 200. `request_log`: non-empty `next_cursor` means older rows - pass it as `cursor`. `conversation_audit`: `has_more` means pass `next_offset` as `offset`; `message_count`/`tool_call_count` are totals. `users`: `next_cursor` is the next `offset`. `usage_by_user`: `truncated` means raise `limit` or narrow the window. To reach an earlier window set `since`/`until`; do not page back to it.

## Rules

- Never conclude a day, total or trend from one page. Use an aggregate tool, or page to the end and say "at least N" if you stopped.
- When two sources disagree, report both figures and do not invent a cause.
- State the window, and name any source that was denied, truncated or empty.

## Running any other command

The `systemprompt` tool runs a CLI command string **without** the `systemprompt` prefix: `{"command": "core skills list"}`. It already sets JSON output and forwards your credential: never pass `--json`, `--yaml`, `--format` or `--export`, and never override the profile or database. Results over 64 KB are truncated - narrow the query rather than repeating it. If a command rejects a flag, run `<command> --help` **once**.

Common health and inventory commands:

| Task | Command |
|------|---------|
| Services status / health | `infra services status`, `infra db status`, `infra db doctor` |
| MCP servers configured / running | `plugins mcp list`, `plugins mcp status`, `plugins mcp logs <server>` |
| Recent errors | `infra logs view --level error --since 1h`, `infra logs trace list --status failed` |
| Skills / plugins | `core skills list`, `core plugins list` |
| Users | `admin users list`, `admin users stats`, `admin users show <id or email>` |
| Jobs | `infra jobs list`, `infra jobs history` |
| Config | `admin config show` (there is no `config gateway show`) |

## Flags that exist

| Command | Flags |
|---------|-------|
| `infra logs request list` | `--since`, `--until`, `--user <id>`, `--model`, `--provider`, `--limit/-n`, `--before <cursor>`; pages by cursor only, no status filter |
| `infra logs audit <id>` | `--messages/-m`, `--tools/-t`, `--offset`, `--limit/-n` (0 = all), `--max-content <chars>` |
| `analytics costs breakdown` | `--by model\|provider\|agent\|user`, `--since`, `--until`, `--limit/-n` |
| `analytics costs summary` / `trends`, `sessions stats` / `trends` | `--since`, `--until` (`trends`: `--group-by`) |
| `analytics requests list` | `--since`, `--until`, `--user <id>`, `--model`, `--limit/-n`, `--offset` |
| `analytics conversations list` | `--since`, `--until`, `--source agent\|gateway\|all`, `--user <id>`, `--limit/-n` |
| `admin users list` | `--limit`, `--offset`, `--role admin\|user\|anonymous`, `--status`, `--include-anonymous` (with `--role` paging is ignored) |
| `admin users role assign <user-id>` | `--roles <role>[,<role>]` - only `admin`, `user`, `anonymous` exist |
| `infra logs governance report` | `--since`, `--group-by policy\|tool\|user`, `--limit` |
| `infra logs trace list` | `--since`, `--agent`, `--status`, `--tool`, `--decision`, `--has-mcp`, `--all`, `--limit/-n` |

Times: `30m`, `24h`, `7d`, or `YYYY-MM-DD[THH:MM:SS]`; `--until` is exclusive. A cursor is `<created_at>@<request_id>`; pass the last row's `cursor` to `--before`.

## Domains

A verb not listed here does not exist.

- **`core`** - `artifacts` (list, show) · `content` · `files` · `contexts` · `skills` (list, show) · `plugins` (list, show, validate, generate) · `marketplace` (explain, import) · `hooks` (list, validate) · `services` (validate, bundle, keygen, publish, refresh, inspect)
- **`infra`** - `services` · `db` (query, tables, describe, info, status, count, size, doctor, migrate*, ...) · `jobs` (list, show, run, history, enable, disable) · `logs` (view, search, stream, summary, show, request {list, show, stats}, trace {list, show}, governance {report}, tools {list}, audit <id>)
- **`admin`** - `users` (list, show, search, create, update, delete, count, stats, role {assign, promote, demote}, session, ban, api-key {issue, list, revoke}; no `role list`) · `agents` · `config` · `session` (show, list, login, logout) · `bridge` (issue-code, list, ...) · `access-control` (export-yaml, lint). No `admin report` command; that is the `admin_report` tool.
- **`analytics`** - `overview` · `conversations` · `agents` · `tools` · `requests` · `sessions` · `content` · `traffic` · `costs` (summary, trends, breakdown). Spend per user is `costs breakdown --by user`; `sessions`, `tools`, `agents`, `conversations` require a verb.
- **`cloud`** - `auth` · `tenant` · `profile` · `deploy` · `backup` · `doctor` · `status`
- **`plugins`** - `list` · `show` · `validate` · `capabilities` · `mcp` (list, status, validate, logs, tools, call)
- **`build`** - `core` · `mcp`

Outside these tables, walk `--help`: `systemprompt <domain> [subcommand] --help`.
