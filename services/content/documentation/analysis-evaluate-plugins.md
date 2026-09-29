---
title: "Evaluate a Plugin: Fixed Suite, PAT Export, Deterministic Metrics"
description: "How to measure a plugin's skills run over run: send a fixed prompt suite through a real client, export every conversation with a personal access token, compute deterministic metrics, and compare versions over time in the console and in git."
author: "systemprompt.io"
slug: "analysis-evaluate-plugins"
keywords: "evaluate, evaluation, eval, plugin, skills, metrics, PAT, personal access token, export, transcripts, versions, compare, baseline, regression"
kind: "guide"
public: true
tags: ["enterprise", "admin", "operations"]
published_at: "2026-09-24"
updated_at: "2026-09-24"
after_reading_this:
  - "Run a plugin's eval suite and export every conversation it produced with a PAT"
  - "Name every metric the eval records and the file it is computed from"
  - "Compare a run against a baseline and read the change in the console's Versions page"
  - "Repeat the loop after each skill change, so improvement is tracked over time"
related_docs:
  - title: "Versions: Marketplace Hashes, History and Compare"
    url: "/documentation/analysis-versions"
  - title: "Analysis: The Record of Every Conversation and Skill"
    url: "/documentation/analysis"
  - title: "Measure Which Skills Are Used"
    url: "/documentation/analysis-measure-skills"
---

# Evaluate a plugin: fixed suite, PAT export, deterministic metrics

**TL;DR:** An eval sends the same prompts to every skill of a plugin through a real, governed client, then exports each resulting conversation with a personal access token (PAT) and computes metrics from the export. Every metric is a count, a sum or a fixed rule over exported files. No model scores anything, so the same export always gives the same numbers. Change the skills, ship them, run the same suite again, and compare. The console's **Versions** page shows the same change by marketplace hash.

## Why this shape

- **Fixed inputs.** Keep the prompts in one versioned file per plugin and stamp its hash on every run. Runs with different prompt hashes are flagged as not comparable, so a change in the numbers is a change in the skills.
- **A real client.** The suite runs in Claude Code through the Systemprompt Internal Bridge, so every turn goes through the gateway, governance, cost accounting and the audit record exactly as a user's would. The eval measures what users get.
- **Deterministic scoring.** Tokens, cost, turns, tool calls and errors come from the gateway's own record of each conversation. Error classes are fixed patterns over tool results. Quality review is done separately by a person or by the engineer changing the skills, and it is never mixed into the metrics.
- **Two histories.** Each run's metrics are kept on the machine that made them, outside git (runs read real users' data). Each shipped change also produces a new marketplace version in the console, which is the shared history.

## The loop

```
edit skills → ship (new image → marketplace hash moves → bridge syncs)
  → run the suite: one headless Claude Code session per skill, through the bridge
  → export each session's conversations with a PAT
  → compute the metrics and compare them with the previous run
  → read the deltas; open Versions → Compare in the console
```

The first run on a plugin is its **baseline**. Every later run is compared with the baseline and with the run before it.

## Run

Run every skill in the suite as a headless Claude Code session on a machine enrolled with the bridge. The prompt is the skill's slash command, the fixed prompt, and a constant suffix that scopes the run to test records and forbids writes and questions. Record, per run:

| Field | Meaning |
|---|---|
| `plugin_version` | The `version` in the plugin config at run time. |
| `prompts_hash` | The hash of the prompt file. Runs are compared only when this matches. |
| `skills` | A content hash per skill directory, so a skill that did not change can be told apart from one that did. |
| `session_id` | The client session ID of each skill's run. It is the key the export uses. |

A run started before the connector tools were ready is a harness retry, not a skill failure; record it as a second attempt rather than as a result.

## Export with a personal access token

A PAT (`sp-live-…`) is accepted on **GET requests to `/admin/export/…` only**. It resolves to its owner and sees exactly what that person's console session would. Any other method or path returns 401. Issue one under **Account → Devices → Personal access tokens**, or use the bridge's own token. The export routes are:

| Request | Returns |
|---|---|
| `GET /admin/export/transcripts?session_id=<client session id>` | JSON Lines, one `ConversationBundle` per conversation in that session (schema version 1). |
| `GET /admin/export/transcripts/{context_id}?format=json\|markdown` | One conversation. |
| `GET /admin/export/transcripts?source=sessions&from=…&to=…` | Every conversation in a window, capped at 500. |
| `GET /admin/export/analysis-skills?from=…&to=…&format=json` | One row per skill: invocations, people, requests, tokens, cost, tool failures, p95. |
| `GET /admin/export/analysis-versions?marketplace=<id>&from=…&to=…&format=json` | One row per marketplace version: served window, invocations, requests, cost, p50/p95. |
| `GET /admin/export/analysis-marketplaces?from=…&to=…&format=json` | One row per marketplace. |

Every dataset also takes `format=csv|json|jsonl|markdown`, and `/preview` returns the row count without the body.

```bash
curl -H "Authorization: Bearer $PAT" \
  "https://<gateway>/admin/export/transcripts?session_id=<id>" -o run.jsonl
```

A bundle carries `facts` (the conversation's record: tokens, cost, turns, latency, tool calls, governance decisions), `requests` (per turn), `tool_calls` and `tool_ledger`, `skills` (each invoked skill with its `plugin_id`, `marketplace_id` and **`marketplace_hash`**), `decisions`, `messages` and `transcript`. Keep the client's local transcript of the same session beside the export if you need full tool results to classify errors yourself.

## The metrics

Every per-run figure is one row of the **`analysis-plugin-eval`** dataset: one row per conversation that invoked one of the marketplace's skills, scored by the fixed rules in `plugin_eval.sql`. The console's **Versions → Evaluation** tab aggregates exactly these rows, so a report built from the export and the page are one set of numbers.

```bash
curl -H "Authorization: Bearer $PAT" \
  "https://<gateway>/admin/export/analysis-plugin-eval?marketplace=<id>&days=30&format=json"
```

Tool calls are read from every turn of the conversation. Each turn repeats the thread before it, so a call is counted once. A call's result is the message after it.

| Column | Rule |
|---|---|
| `context_id`, `client_session_id` | The conversation, and the client session that ran it (a run keys on this). |
| `skill`, `plugin_id`, `marketplace_hash` | The skill invoked and the marketplace version that served it. |
| `turns`, `requests`, `input_tokens`, `output_tokens`, `cache_read_tokens`, `cache_creation_tokens` | The conversation's record. |
| `cost_usd` | Turn spend, excluding side calls (titles, summaries). |
| `duration_seconds`, `p50_ms`, `p95_ms` | Wall time, and median and p95 request latency. |
| `mcp_calls`, `connector_calls`, `builtin_calls` | MCP tool calls; calls to the plugin's own MCP servers; built-in tool calls. |
| `schema_errors` | Results with `INVALID_FIELD`, `No such column`, `No such relation`, `INVALID_TYPE`, `is not supported`, `MALFORMED_QUERY` or `ValidationError`. |
| `access_errors` | `INSUFFICIENT_ACCESS`, `Permission '…' denied` or `PERMISSION_DENIED`. |
| `upstream_errors` | A 5xx `status_code` or `UNKNOWN_EXCEPTION` from the connector's backend. |
| `bad_arguments` | The tool rejected the call's shape: `Invalid JSON payload`, `Invalid value at '…'`, or a name that does not match its pattern. |
| `timeouts` | `timed out`. |
| `failed_calls` | Calls whose result is in any class above. |
| `repeated_calls` | Calls re-issued with identical input: retry loops. |
| `widgets` | `display_widget` calls that rendered. |
| `writes` | Calls to a create, update, delete, upsert, send or insert tool, or with a POST, PATCH, PUT or DELETE method. |
| `answer_chars` | Length of the final answer. |
| `placeholder_mentions` | `MOCK`, `placeholder` or `sample data` in the final answer: output built on invented data. |
| `tools_unavailable` | The final answer says a tool or connector was unavailable: the run never reached its connector. |
| `completed` | A widget rendered, or the final answer is at least 200 characters. |
| `success` | Completed, at least one call to the plugin's connector, no schema errors, no writes, and tools were available. |

**`analysis-plugin-eval-tools`** returns one row per version, skill and tool: calls, conversations, failed calls by class and repeated calls, by the same rules. It names the tool that is failing, not just the skill.

### Levels

| Level | Where |
|---|---|
| Marketplace version | The Evaluation tab's version table; `marketplace_hash` in both exports. |
| Plugin | The tab's plugin table; `plugin_id`. |
| Skill | The tab's skill table; `skill`. |
| Tool | The tab's tool table; `analysis-plugin-eval-tools`. |
| Conversation | Each row of `analysis-plugin-eval`; each skill links to its latest run. |
| Day | The tab's day table for the selected version; `first_invoked_at`. |

### Attention list

The top of the tab lists what needs a look, by fixed thresholds:

| Flag | Rule |
|---|---|
| Wrote to a connector | Any write call. |
| Ran without its tools | Any run with `tools_unavailable`. |
| Succeeds less often | The skill's success rate is below its rate under the baseline version. |
| Costs more per run | The skill's average cost is 25% or more above the baseline. |
| Repeats calls | A run re-issued the same call 3 or more times. |
| Tool fails | A tool with 3 or more calls fails 20% or more of them. |

Every item links to the latest conversation that shows it.

## Read the change in the console

- **Analysis → Versions → the plugin's marketplace → Evaluation.** Pick a version, and optionally a version to compare with (by default, the previous version that has conversations). The headline shows success, completion, cost, tokens, turns, connector calls and failed calls, each with its change. Below it:
  - a table of every version with conversations, oldest first, each compared with the one before it;
  - each skill under the selected version, compared with the same skill under the baseline, linked to its latest conversation.

  **Export** on this tab downloads `analysis-plugin-eval` for the same window.
- **History** and **Compare.** Usage, spend and latency per version, and what changed between two versions skill by skill.
- **Analysis → Conversations.** Open any run by its `context_id` to read the transcript and tool ledger.

A run made before a deploy is served by the old marketplace version, so the console lists it under that version until the deploy.

## Repeat

After each change, run the same suite with a new label, export it, and compute the metrics against the baseline and against the previous run. Runs stay on the machine that made them, out of git, because transcripts and per-case results describe real users' work. The console's Evaluation tab is the shared record.
