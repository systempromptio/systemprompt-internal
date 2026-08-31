# Report

The standing report for the control plane, answering three questions: **who** is on the platform,
**what** it did recently, and **how** the fleet is doing overall. For drilling into one request or one
conversation, hand off to `inspect`.

## Ask me things like

- "Who signed up this week?" / "List the admins."
- "What's happened in the last hour?" / "What failed today?"
- "Where is our spend going?" / "How is the whole system doing?"

## How commands run

Everything runs through the admin `systemprompt` MCP server's single `systemprompt` tool, taking one
`command` argument — the CLI command **without** the `systemprompt` prefix:

```json
{ "command": "admin users list --limit 50 --json" }
```

Admin-only; a non-admin gets `Insufficient permissions. User must have one of: ["admin"]`, which is
the gate working.

## Output format — pick one deliberately

- **Answering in chat** — omit `--json`. Summarise in prose plus a markdown table of the rows that
  matter. Never paste raw CLI output wholesale.
- **Feeding a dashboard artifact** — always add `--json`. The CLI emits a typed envelope:
  `{"x-artifact-type":"table","columns":[{"name","column_type"}],"items":[{...}]}` for lists, or
  `{"x-artifact-type":"presentation_card","sections":[{"heading","content"}]}` for stats, or
  `message` when there is nothing to report. (`x-artifact-type` is the canonical tag; envelopes also
  carry a redundant `artifact_type` serde tag.) If the matching dashboard is installed, prefer
  refreshing it over re-pasting rows.

## Time windows

`--since` accepts `30s`, `1h`, `24h`, `7d`, `30d`, defaulting to `24h` on the analytics commands; most
also take `--until`. **Always state the window you used** — "9 requests in the last hour" and "9
requests in the last 30 days" are very different findings.

---

# 1. Who — the roster

```bash
systemprompt admin users list --limit 50                    # newest first
systemprompt admin users list --role admin                  # admin | user | anonymous
systemprompt admin users list --status suspended            # active | inactive | suspended | pending | deleted | temporary
systemprompt admin users list --limit 50 --offset 50        # page 2
systemprompt admin users count
systemprompt admin users stats                              # totals, created_24h, by-role and by-status splits
```

`list` returns `id`, `name`, `email`, `status`, `roles`. **Read `roles` before reporting a headcount:**
most workspaces carry a large tail of `anonymous` users minted per browser fingerprint
(`fp_*@anonymous.local`). Those are visitors, not accounts. Give the signed-in count (`--role user`
plus `--role admin`) and mention the anonymous tail separately, rather than quoting the raw total.

### One account

```bash
systemprompt admin users show <user-id>
systemprompt admin users search "<name | email>"            # start here when you only have a name
systemprompt admin users session list --user <user-id>
systemprompt admin users webauthn list --user <user-id>     # registered passkeys
systemprompt admin users export --json                      # full records, offline analysis
```

### Roles

```bash
systemprompt admin users role list <user-id>
systemprompt admin users role promote <user-id>             # grant admin
systemprompt admin users role demote <user-id>
systemprompt admin users role assign <user-id> --roles user,admin
```

A role change takes effect on the **next token issue**, not immediately: scopes are minted when the
session token is issued. After promoting someone, tell them to sign out of the bridge and back in, or
their admin surfaces keep returning permission errors. To watch governance itself honour a role flip
live, that is `manage_platform`.

`promote`, `demote`, `update`, `delete`, `merge`, and `bulk` all mutate accounts. Confirm the target
with `show` first, state plainly what you are about to change, and get the go-ahead before running.

---

# 2. What — recent activity

### Live right now

```bash
systemprompt analytics sessions live                  # sessions active this moment
systemprompt analytics sessions stats --since 24h
systemprompt analytics conversations list --limit 20
```

### Recent requests

```bash
systemprompt infra logs request list --limit 50       # one row per /v1/messages hit
systemprompt infra logs request list --since 1h --provider anthropic
systemprompt infra logs request stats
```

Columns: `request_id`, `timestamp`, `provider`, `model`, `tokens`, `cost`, `latency_ms`, `status`. No
`--status` filter here — that lives on `trace list`. To explain a single row, hand the `request_id` to
`inspect`.

### What failed

```bash
systemprompt infra logs view --level error --since 1h
systemprompt infra logs trace list --status failed --limit 20
systemprompt infra logs trace show <trace-id>
```

`trace list` covers MCP tool calls (PreToolUse → decision → spawn → result); `request list` covers
gateway inference. Different paths through the same audit spine — check both before concluding
"nothing failed".

---

# 3. How — the fleet

### Overview and cost

```bash
systemprompt analytics overview                       # all domains, period-over-period (takes no verb)
systemprompt analytics costs summary                  # total spend, requests, tokens, avg cost/request
systemprompt analytics costs breakdown --by model     # or: --by agent, --by provider
systemprompt analytics costs trends --since 7d
```

### Requests, tools, sessions, content

```bash
systemprompt analytics requests stats                 # volume, tokens, latency, cache hit rate
systemprompt analytics requests models                # per-model breakdown (routing mix)
systemprompt analytics tools stats                    # executions, success rate, p95 latency
systemprompt analytics tools show <tool-name>
systemprompt analytics content top                    # top performing content
systemprompt analytics traffic sources                # referrers / channels (also: geo, devices, bots)
```

Each topic needs a verb and the verbs differ per topic — not every topic has every verb:

| Topic | Verbs |
|-------|-------|
| `costs` | `summary`, `trends`, `breakdown --by {model,agent,provider}` |
| `requests` | `stats`, `list`, `trends`, `models` |
| `agents` | `stats`, `list`, `trends`, `show <agent>` |
| `tools` | `stats`, `list`, `trends`, `show <tool>` |
| `sessions` | `stats`, `trends`, `live` |
| `conversations` | `stats`, `trends`, `list` |
| `content` | `stats`, `top`, `trends` |
| `traffic` | `sources`, `geo`, `devices`, `bots` |

`systemprompt analytics <topic> --help` is authoritative when unsure.

## Reading attribution — read this before reporting agent numbers

Two traffic paths attribute differently:

- **Gateway inference** — every `/v1/messages` hit. Lands in `ai_requests` and drives `costs` and
  `requests`, but carries **no agent task**, so it shows as `unattributed` under
  `costs breakdown --by agent` and produces **no rows** in `agents list`.
- **Agent tasks** — spawned agent runs and their MCP tool calls. These populate `agents` and `tools`.

This instance ships no A2A agents, so `costs breakdown --by agent` reading 100% `unattributed` and
`agents list` saying "No agents found" is the **expected** shape, **not** a tracking failure. Do not
report it as one. Say "spend is gateway inference, not attributed to named agents", and pivot to
`--by model` / `--by provider` and `requests models`. Only call attribution broken if agent tasks
*were* run in the window and still do not appear.

---

## Typical workflow

1. `admin users stats` — headcount, split by role and status, signups in the last 24h.
2. `analytics sessions live` — is anyone on right now?
3. `infra logs request list --since 24h` — recent traffic with model, cost, latency, status.
4. `infra logs trace list --status failed` + `infra logs view --level error --since 1h` — what broke.
5. `analytics costs breakdown --by model` — where the money went (mind the attribution note).
6. Drill into anything anomalous with `inspect`.
7. Close with a short verdict: volume, spend, error rate, and the one thing worth acting on.

---

## Running it as the demo close

DEMO.md step 5 is this skill, sequenced — the finale, and the reason the earlier steps kept reading
costs back: the "cheaper, faster" claim becomes provable from this installation's own metered data.
Agentforce answers the same question with a credit statement priced per conversation; here every task
decomposes into requests, tokens, tools, and dollars, queryable to the row.

1. **Business close** — `crm_lead_report` grouped by stage: the pipeline as it stands after the
   demo's writes.
2. **Fleet view** — the commands from §3 above, in this order, narrated as one paragraph covering
   spend, request count, cache-hit rate, tool reliability, and latency:
   ```bash
   systemprompt analytics overview --since 24h
   systemprompt analytics costs summary
   systemprompt analytics costs breakdown --by model
   systemprompt analytics tools stats
   systemprompt analytics requests stats
   ```
3. **Dashboards** — open the three admin artifacts installed by `systemprompt_setup_admin`:
   **Usage & Costs**, **Activity & Requests**, **Users Directory**. Same data plane, standing views.
4. **The bill for this demo** — total what the audience just watched:
   ```bash
   systemprompt infra logs request list --since 2h
   systemprompt analytics costs trends --since 24h
   ```
   Sum the session's requests and state it plainly: *"Everything you just saw — a pipeline briefing,
   CRM writes, three governance denials, and this report — cost $X.XX."*
5. **The comparison** — set that number against Agentforce's list pricing (approximately $2 per
   conversation on Flex Credits, per Salesforce's published pricing). **Label it as list price**, not
   a measurement, and let the audience correct it if they have negotiated rates. Typically the whole
   demo costs less than one Agentforce conversation. Add the structural points: self-hosted (flat
   infra cost, no per-seat Einstein SKUs), per-user identity, and an audit row for every cent.

Every number spoken must come from a command run in this session — the comparison only lands if our
side of it is verifiably real. Never estimate our costs; the rules in `governance_readback` apply.
