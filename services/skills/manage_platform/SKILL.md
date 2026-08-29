# Manage Platform

Operate the running stack — services, database, scheduled jobs — and change who is allowed to do what.
Both halves mutate live state, so both carry the same rule: say plainly what you are about to change
and get the go-ahead before running it.

## Ask me things like

- "Bring the stack up." / "What's running?" / "Is the database migrated?"
- "Grant this person admin." / "Show me a permission flip take effect live."

---

# 1. Services, database, jobs

### Services

`infra services` manages the API server, agents, and MCP servers together.

```bash
systemprompt infra services start            # start API, agents, and MCP servers
systemprompt infra services status           # which are running, PIDs
systemprompt infra services restart          # restart all
systemprompt infra services restart api      # reload just the API server's config
systemprompt infra services stop             # stop gracefully
systemprompt infra services cleanup          # clear orphaned processes / stale entries
```

### Database

The local stack runs a per-clone Docker Postgres.

```bash
systemprompt infra db status                 # connection status
systemprompt infra db tables                 # tables with row counts and sizes
systemprompt infra db migrations status      # migration status across extensions
systemprompt infra db query "SELECT ..."     # read-only SQL
systemprompt infra db migrate-repair         # fix migration checksum drift in place (no data loss)
```

Container lifecycle belongs to the justfile: `just db-up`, `just db-down`, `just db-logs`. **There is
no destructive reset** — recover drift in place with `infra db migrate-repair` (or
`just repair-migrations`).

### Jobs

```bash
systemprompt infra jobs list                 # available jobs
systemprompt infra jobs run <job>            # run a scheduled job manually
systemprompt infra jobs history              # execution history
```

`publish_pipeline` also runs automatically at server startup.

### Logs while operating

```bash
systemprompt infra logs stream --since 30s            # live tail (alias: follow)
systemprompt infra logs view --level error --since 1h
```

**Workflow:** `infra services status` → `infra db status` → `infra services start` →
`infra logs stream` to watch startup and catch errors.

---

# 2. Permissions, and proving they are live

Change what a user may do, then watch the governance pipeline honour it on the very next call — no
restart, no new token. This is the live proof that authority is data, not a baked-in constant.

## Why it works

The governance `scope_check` policy decides admin-only tools (`mcp__systemprompt__*`) by reading the
caller's roles **from the database on every request** (`users.roles`), not from a cached token claim.
So editing roles with `admin users role …` takes effect on the **next** governance decision, with **no
reload, no service restart, and no re-issued token** — the same bearer token flips outcome. (A minted
plugin token carries `scope=hook:govern hook:track`, so the DB role, not the token, decides admin
access.) `admin` role → Admin scope, all tools. Anything else → User scope, admin-only tools denied.

## Safety: never target the configured system admin

The profile designates one system admin (here `ed`). Demoting that user trips a startup guard
("system admin exists but does not carry the admin role") that blocks CLI access. **Always run this
against a dedicated throwaway user.** The sequence below creates one and deletes it at the end.

### 1. Create a demo user and mint its token

`issue-plugin-token` refuses non-admin users, so promote first, mint while the user is admin, then
flip the role. The token stays valid; only the DB role changes.

```bash
systemprompt admin users create --name perms_demo --email perms_demo@demo.local --if-not-exists
systemprompt admin users role promote <user_id>
systemprompt admin keys issue-plugin-token --email perms_demo@demo.local   # copy the JWT
```

Hold that token as `$TK`.

### 2. Revoke admin — the same request is now DENIED

```bash
systemprompt admin users role demote <user_id>

curl -s -X POST "http://localhost:8080/api/public/hooks/govern?plugin_id=enterprise-demo" \
  -H "Authorization: Bearer $TK" -H "Content-Type: application/json" \
  -d '{"hook_event_name":"PreToolUse","tool_name":"mcp__systemprompt__users_show","session_id":"perms-deny","tool_input":{}}'
# -> {"permissionDecision":"deny", reason: "tool mcp__systemprompt__users_show requires admin"}
```

### 3. Grant admin — the SAME request is ALLOWED

```bash
systemprompt admin users role promote <user_id>

curl -s -X POST "http://localhost:8080/api/public/hooks/govern?plugin_id=enterprise-demo" \
  -H "Authorization: Bearer $TK" -H "Content-Type: application/json" \
  -d '{"hook_event_name":"PreToolUse","tool_name":"mcp__systemprompt__users_show","session_id":"perms-allow","tool_input":{}}'
# -> {"permissionDecision":"allow"}
```

The only thing that changed between the deny and the allow is one CLI command. Same token, same tool,
same endpoint.

### 4. Confirm in the audit trail, then clean up

```bash
systemprompt infra db query "SELECT decision, tool_name, policy, reason, created_at FROM governance_decisions WHERE session_id IN ('perms-deny','perms-allow') ORDER BY created_at"
systemprompt admin users delete <user_id>
```

Two rows seconds apart for the same user: one `deny` from `scope_check`, one `allow`.

The takeaway: permissions are live state. One command revokes or re-grants authority, and the next
governed action reflects it immediately — which is exactly what an auditor needs to see.
