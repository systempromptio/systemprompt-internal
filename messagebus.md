# The Message Bus

Real-time communication between our people and their agents, on our own
governed infrastructure. No Slack.

> Implementation plan: `~/.claude/plans/we-have-all-of-concurrent-summit.md`.
> Build status: §13.

---

## 1. The problem, stated precisely

Everyone works through Cowork or Codex against this backend. There is no
channel: teammates can't message each other, agents can't hand off, and a
long-running agent has no way to tell its human that it is stuck.

The obvious build — a table plus a "you have mail" hook — fails on the
requirement that matters most: **a message must not appear in every conversation
that happens to be running.** An agent mid-refactor in one repo should never be
interrupted by chatter meant for a different workstream. Delivery therefore has
to be *addressed*, and addressing requires the backend to know which sessions
exist and what each one is doing.

It did not know that. That gap is the first half of this document.

---

## 2. What the backend knew about a session

There is **no `sessions` table for agent sessions.** A Claude Code session was
represented by two unlinked things:

| Thing | Where | What it is |
|---|---|---|
| `user_sessions` | core, `crates/domain/users/schema/user_sessions.sql` | web/OAuth **analytics** session. Not an agent session. |
| `bridge_sessions` | core, `crates/domain/oauth/schema/bridge_sessions.sql` | per-machine bridge presence — `last_heartbeat_at`, `hostname`. |
| `plugin_session_summaries` | `extensions/web/schema/07_analytics.sql:42` | **the Claude Code session row.** Keyed on Claude Code's own uuid. |

Claude Code's session uuid never becomes a `user_sessions` row. It arrives raw
as `payload.session_id` and keys `plugin_session_summaries`,
`plugin_usage_events`, and `governance_decisions`. The link to a person comes
from the JWT `sub`, not from the session id.

### What we already received and threw away

Every hook event carries `HookCommonFields`
(`extensions/web/admin/src/types/webhook/event_types.rs:19`) — `session_id`,
`cwd`, `permission_mode`, `transcript_path`, `agent_id`. And four capabilities
were inert:

1. **`/hooks/statusline` was a stub** — authenticated, returned 204, `_payload`
   unbound. It discarded `model.api_model_id`, `cost.total_cost_usd`, and
   `context_window.current_usage` — the richest live signal we have.
2. **`/hooks/transcript` is a stub** too. `session_transcripts` has no writer.
3. **`EventHub::notify(user_id)`** fires on every ingested hook event
   (`handlers/hooks_track/processing.rs:71`). **Nothing calls `subscribe`.**
4. **`PrincipalSnapshot::agent_session`** is hardcoded `None` everywhere and has
   no DB column.

`plugin_session_summaries` had no `cwd`, no `last_event_at`, no notion of
current activity. `/admin/entities/sessions` rendered the viewer's own JWT and
queried nothing.

---

## 3. Layer 1 — The Session Registry

**This is the "decorative metadata" layer, and it must earn its place
independently of messaging.** It does, on signals we already receive.

### Schema

Extend `plugin_session_summaries` rather than adding a parallel table — it is
already the session row, already upserted on every event, already joined by the
session-detail UI. Migration `030_session_registry.sql`:

| Column | Source | Also useful for |
|---|---|---|
| `cwd` | `HookCommonFields.cwd`, already on every event | attributing cost to a repo |
| `workspace` | `basename(cwd)`, computed on upsert | grouping sessions by project |
| `git_branch` | new — needs a bridge change (§13) | tying spend to a branch/PR |
| `handle` | derived, see below | **the address** |
| `last_event_at` | upsert timestamp | liveness, stale-session reaping |
| `current_activity` | last prompt preview / tool name | the live board |
| `live_cost_microdollars` | statusline | real-time spend, not Stop-time |
| `context_pct` | statusline `context_window` | sessions about to compact |

Cost is `BIGINT` microdollars to match `ai_requests.cost_microdollars`; a float
dollar column would silently disagree with every existing rollup.

### Handles — how a session becomes addressable

Raw uuids are unusable by hand. Derive a stable, human-readable handle:

```
<workspace>       systemprompt-internal
<workspace>#2     second concurrent session in the same workspace
```

Uniqueness is a **partial** unique index on `(user_id, handle)` over live
sessions only, so a handle is reclaimed when its session ends rather than being
consumed forever. Assignment races other sessions claiming the same base and
retries on conflict, rather than reading-then-writing.

### Standalone value (worth building even if messaging is cancelled)

- A real **live-sessions view** — who is working, on what repo, how much it has
  cost so far, how close to compaction. This page previously showed none of it.
- **Cost per workstream.** `ai_requests` already carries `session_id`; joining a
  registry with `workspace` turns spend into per-repo attribution for free.
- **Stale-session reaping** — `last_event_at` gives `count_concurrent_sessions`
  something honest to count.
- **Governance forensics** — `governance_decisions.session_id` becomes
  resolvable to a repo instead of a bare uuid.

---

## 4. Layer 2 — Messages

### Schema — `extensions/mcp/comms/schema/01_comms.sql`

```
comms_channels          id, slug, name, description, required_role, urgent, created_by
comms_channel_members   channel_id, user_id, muted, joined_at
comms_messages          id, channel_id, sender_user_id, sender_session_id, sender_handle,
                        recipient_user_id, recipient_session_id,
                        delivery_class, body, thread_id, created_at
comms_reads             user_id, session_id, scope, last_read_at
```

`comms_reads` keyed on `(user_id, session_id, scope)` is what makes "unread"
per-agent rather than per-person: two of Ed's sessions each track their own mark.

### Addressing

| Form | Resolves to | Interrupts? |
|---|---|---|
| `@ed` | the person | **No** — inbox only |
| `@ed/odoo-crm` | one session, via registry handle | Yes, that session only |
| `#crm` | channel subscribers | No — inbox only |

A bare word is rejected rather than guessed at: reading `crm` as `@crm` would
deliver a channel post to a person.

### Delivery classes — the anti-spam guarantee

> **A message enters a running conversation only if it names that session.**

- **`inbox`** (default for `@user` and `#channel`) — stored, raises an unread
  count, and does *nothing else*.
- **`session`** — addressed to a specific handle. That session and no other.
- **`urgent`** — every live session of the recipient. Reserved for urgent
  channels and governance holds.

Presence-aware degradation: a `session`-class message naming a handle with no
recent `last_event_at` silently becomes `inbox`. Nobody has to know whether a
peer is online before writing to them.

---

## 5. Layer 3 — Delivery mechanics

```
comms_send (MCP tool, port 5060)
        │
        ▼
comms_messages row                        ← durable, audited
        │
        ├─ class=inbox   → stop here. Unread count only.
        │
        └─ class=session/urgent
                │
                ▼  EventRouter::route_agui(recipient_user_id, custom payload)
                   → outbox row + NOTIFY (cross-replica) → broadcasters
                │
                ▼  SSE, one connection per teammate
           bridge daemon on the recipient's machine (127.0.0.1:48217)
                │
                ▼  writes inbox/<session_id>.jsonl — one file PER SESSION, so a
                │   wrong-session message is never on disk for the wrong hook
                │
                ├─► UserPromptSubmit hook → additionalContext → visible next turn
                ├─► Stop hook (async)     → visible at turn end for a busy agent
                └─► Notification hook     → OS notification for an idle human
```

**Why each leg exists:**

- **MCP tools are pull.** Necessary for reading and replying; never sufficient
  for push.
- **SSE is core's, and it is already complete.** Four routes are mounted and
  authed, including `GET /api/v1/stream/contexts`, with connection registration,
  a drop-guard, a 15s keep-alive, and a 10-connection-per-user cap.
  `PostgresEventBridge` (cross-replica LISTEN/NOTIFY relay, outbox pruning,
  retry backoff, standby detection) starts at boot. Nothing needs building for
  fan-out. Do **not** build on `extensions/web/admin/src/event_hub.rs` — it is a
  redundant per-user broadcast with no subscribers.
- **Hooks are the only true push into Cowork.** The signed manifest ships
  `HookEntry { event, matcher, command, is_async }`, and `HookEvent` includes
  `UserPromptSubmit`, `Stop`, `Notification`. Because the bridge has already
  written the file, the hook is a local read — it never adds latency to a turn.

**Honest latency contract:** a message is on the recipient's disk in under a
second; it becomes *visible* at the next hook boundary — their next prompt, or
when the current turn ends. That is real-time for a working team. It is not a
chat window with a typing indicator, and the docs should say so.

What is genuinely missing is only on the **client** side: the bridge has no
long-lived connection to the server. Its sync agent is a *scheduled task*
(launchd / Task Scheduler / systemd), and liveness is HTTP
`POST /v1/bridge/heartbeat`. The consumer therefore belongs in the one bridge
component that is a daemon — the proxy on `127.0.0.1:48217`.

---

## 6. Layer 4 — Surfaces

### 6.1 Why MCP tools are the only data path into an artifact

A Cowork artifact **cannot** fetch our JSON API, for three independent reasons:

- **CSP** — a rendered MCP UI panel is `connect-src 'self'`.
- **CORS** — core's layer is an explicit allowlist with no wildcard; Cowork's
  artifact origin is not on it.
- **Cookies** — auth is a `SameSite=Lax`, `HttpOnly` cookie, not sent from a
  cross-site iframe, and artifact JS cannot read it.

`window.cowork.callMcpTool` is the supported path, and it executes **as the
signed-in user**. That is a feature: the inbox artifact cannot show one teammate
another's messages even if the HTML is wrong.

### The MCP server — `extensions/mcp/comms`, port 5060

| Tool | Returns | Read-only |
|---|---|---|
| `comms_send` | `Message` | no |
| `comms_inbox` | `List` | yes |
| `comms_history` | `Table` | yes |
| `comms_channels` | `Table` | yes |
| `comms_sessions` | **the directory** — live handles | yes |

`RequestContext` already gives `user_id()` and `session_id()`, so every tool
knows who and which session is calling. `annotations.readOnlyHint: true` on the
reads, because Cowork only caches dashboard MCP results when it is advertised.

### The human inbox — a Cowork artifact

`services/artifacts/team-inbox/` calls `window.cowork.callMcpTool`, the pattern
proven by `services/artifacts/recent-activity/view.html:265`. It passes
`peek: true` deliberately: a dashboard render must not advance a session's read
mark, or opening the page would hide messages from the agent they were addressed
to.

### Codex gets a different surface — and this is a real constraint

**Codex CLI has no `callMcpTool` bridge**: inline visualizations reject the call
with "Inline visualizations cannot call tools." The artifact board is Cowork-only.
Codex gets the inline `CliArtifact` panel and the text summary — which is why
every handler's `String` summary is load-bearing and must read well alone.

---

## 7. Use cases

1. **Async note between teammates.** `@sam` → inbox. Nothing interrupted.
2. **Redirecting your own other session.** `@ed/systemprompt-core "schema
   changed, re-run prepare"` lands in that session and only that one. This is the
   case that makes handles worth building.
3. **Agent escalation to its human.** `@ed` → inbox, answered when they look.
4. **Governance approval holds — the strongest case.** The fifth stage,
   `require_approval`, returns `Decision::Pending` and parks the call on an
   `approval_requests` row while a named human resolves it. Today nothing tells
   that human. The bus makes the hold notify them, dropping approval latency
   from "whenever they check the dashboard" to seconds.
5. **Agent-to-agent handoff**, governed and audited on both sides.
6. **Channel broadcast** — `#crm`, inbox class, never interrupts.
7. **Long-running job completion** — scheduler jobs already run under a named
   `owner`; a finished job messages that owner instead of a log nobody reads.
8. **Cross-machine** — routing is by `user_id` through the cross-replica outbox.

---

## 8. Governance

The MCP server plane runs RBAC only. Agent-initiated `comms_send` calls are
already governed at the hook plane (`POST /hooks/govern`), inheriting
secret-scan, blocklist and rate-limit coverage. **Do not add a second in-server
chain call** — `GovernanceEngine::global()` is a process singleton precisely to
stop the rate limiter being double-counted.

A welcome consequence: the secret-scan stage means a teammate cannot paste a
live API key into a channel.

The `require_approval` stage matches tool names by **substring** against
`channel_post`, `note_add`, `email_send`. None of the five comms tool names
collide — checked, not assumed.

---

## 9. Files

| Path | Change |
|---|---|
| `extensions/web/schema/migrations/030_session_registry.sql` | registry columns |
| `extensions/web/admin/src/repositories/dashboard/session_registry.rs` | handle assignment, activity, statusline |
| `extensions/web/admin/src/repositories/analytics/live_sessions.rs` | board + workspace costs |
| `extensions/web/admin/src/handlers/webhook/tracking.rs` | statusline implemented |
| `extensions/web/admin/src/handlers/ssr/ssr_users_sessions.rs` + `.hbs` | the live board |
| `extensions/mcp/comms/**` | the MCP server |
| `services/mcp/comms.yaml`, `services/access-control/roles.yaml` | declaration + grant |
| `services/artifacts/team-inbox/` | the board artifact |
| `../systemprompt-core` | Phase 3 only: `/v1/bridge/stream`, bridge proxy consumer |

---

## 10. What has to change in core

Far less than expected, because the event pipeline is already built.

### 10.1 Nothing for fan-out

`SystemEvent` is a closed enum; adding a variant would break every exhaustive
match. Core already solves this: `AgUiEvent` carries
`CustomPayload::Generic { name, value }`, exactly as core itself uses for
`notifications/messageAdded`. Carry **ids and a preview only, never the body** —
the client fetches through the MCP tool, so SSE never becomes an unaudited data
path.

### 10.2 The one real core change — a bridge-authenticated stream route

`/api/v1/stream/*` sits behind `UserOnlyContextMiddleware`, whose
`TokenExtractor::browser_only()` accepts Bearer JWT or cookie but **not
`x-api-key`** — which is how the bridge authenticates. Mount
`GET /v1/bridge/stream` reusing `create_sse_stream` with `extract_credential` +
`decode_for_gateway`. ~40 lines, additive, no existing route touched, and it can
filter server-side to `comms.*` rather than shipping every event to every laptop.

### 10.3 Bridge — an SSE consumer in the proxy daemon

`bin/bridge/src/proxy/comms.rs` (note: `bin/`, not `crates/`). Hook installation
reuses `bin/bridge/src/sync/apply/hooks.rs`, which already writes the client
hooks file — no new install mechanism. Costs a `min_bridge_version` bump and a
four-platform `bridge-v*` release.

### 10.4 Optional while in there

Populate `PrincipalSnapshot::agent_session` and add the missing
`governance_decisions.agent_session` column.

---

## 11. What this unlocks for reporting

- **There was no per-repo attribution anywhere in the product.** Zero grep hits
  across core and internal. Yet `cwd` arrives on every hook and was read in
  exactly one place — to interpolate a repo name into an AI summary sentence,
  then discarded.
- **`/admin/entities/sessions` was a dead page.** The registry doesn't improve
  it; it creates the product's only live operational view.
- **It fixes a real bug.** APM fell back to `duration_minutes = 1.0` when
  `ended_at` was null, so every crashed or interrupted session was scored as if
  all its work happened in one minute — inflating `apm`/`eapm` and every daily
  rollup averaging them.
- **In-flight vs post-hoc cost.** `ai_requests` gives cost *after* the fact;
  statusline gives it *during*. That is the difference between reporting a budget
  overrun and enforcing one against `OrganizationMonthPnl.cap_microdollars`.
- **`analytics sessions live` is web traffic**, not agent sessions — every query
  reads `v_clean_traffic`. The registry gives that command an honest
  implementation.
- **The bus adds its own metrics**: escalation rate, human response latency, and
  approval wait time on `require_approval` holds — the one number that says
  whether the fifth stage is helping or just blocking.

---

## 12. Sequencing

1. **Session registry** — internal only, ships value alone. **Done.**
2. **Comms, pull-only** — internal only, correct addressing, no bridge
   dependency. **Done.**
3. **Push** — the only cross-repo work. Not started.

---

## 13. Build status (as landed)

**Phase 1 — session registry.** Migration `030` adds eight columns and backfills
`last_event_at`. Handles are assigned on ingest; the statusline stub is
implemented (with user attribution fixed — it discarded its JWT claims);
the APM fallback now uses `COALESCE(ended_at, last_event_at)`.
`/admin/entities/sessions` is the live board, with a per-workspace cost table.

**Phase 2 — comms, pull-only.** `extensions/mcp/comms` on **port 5060**, not
5050: email already held that port and had published it across the demo docs.
Five tools, four tables, and the `team-inbox` artifact bundled through
**`systemprompt-crm`** — not `systemprompt-commons` as §9 first anticipated,
because commons is disabled for the MVP marketplace and the sync script only
bundles crm and admin. `knowledge-feed` sits there for the same reason.

**Two deviations from the plan, both deliberate:**

- The handle is **workspace-only**, not branch-qualified. A handle that changed
  on `git switch` would break every reference to it mid-conversation, which
  defeats the point of addressing. `git_branch` is stored and displayed, not
  addressed.
- `git_branch` has **no writer yet**. The plan assumed a `SessionStart` hook
  script could supply it, but hooks here are not scripts: the plugin sets
  `hooks: governance: true`, a flag the *bridge* turns into a hooks file from a
  fixed template. Populating it is Phase 3 work.

**Verified:** `just build` green; 523/523 unit tests; 12 addressing tests; 8
handle-derivation tests; `admin config validate` passes. Handle uniqueness,
reclaim-on-end, and — the one that matters — **inbox isolation** were proven
directly against the schema: a session sees its own messages, its user's
unaddressed messages, and its channels, but **not** a sibling session's
messages; a different user sees nothing. Every lint gate passes except
`check-fork-drift`, which also fails at HEAD.

**Phase 3 — push.** Implemented across all three repos:

- **Fan-out** (`extensions/mcp/comms/src/server/tool/fanout.rs`) — announces via
  `EventRouter::route_agui` with a `comms.message` custom payload. No core model
  change. Carries ids and a 120-char preview, never the body. `inbox` class is
  deliberately silent: announcing it would reintroduce the interruption the
  classes exist to prevent.
- **Core route** (`crates/entry/api/src/routes/gateway/bridge_stream.rs`) —
  `GET /v1/bridge/stream`, authenticated with `extract_credential` +
  `decode_for_gateway` so the bridge keeps one consistent auth story.
- **Bridge consumer** (`bin/bridge/src/proxy/comms.rs`) — reconnecting SSE
  client in the proxy daemon with 1s→60s backoff, writing
  `inbox/<session_id>.jsonl`. One file per session: a message addressed
  elsewhere is not filtered out, it is never written where the wrong hook could
  find it.
- **Hooks** — `UserPromptSubmit` (sync) and `Stop` (async), materialised into
  `hooks.json` beside the governance hooks and owned by the same plugin, so
  there is one drain per boundary rather than one per installed plugin. The hook
  command is `<bridge-binary> comms-drain`
  (`bin/bridge/src/cli/comms_drain.rs`), not a shell script — which removes the
  macOS/Linux/Windows portability problem and reuses the code that knows where
  the inbox lives.

**Verified end to end.** `tests/e2e/src/comms.rs` — five tests against a real
throwaway database and the real `systemprompt-mcp-comms` binary over the MCP
wire, ~20s each. The isolation property is asserted where it matters: two
sessions of the same user, the same token, the same tool, and the sibling does
not see the message.

**Remaining:** the bridge changes need a four-platform `bridge-v*` release and a
`min_bridge_version` bump before push works on anyone's machine. Nothing has run
against a real Cowork session yet — the wire is built and tested, the delivery
into a live client is not proven.
