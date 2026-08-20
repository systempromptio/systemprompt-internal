# Testing the Odoo sign-in flow with the MCP Inspector

Email-ready instructions for testing Systemprompt Internal's OAuth flow, where
Odoo is the identity provider. Any user who exists in the local Odoo can sign
in to the platform; their platform account is created automatically on first
sign-in (JIT registration — there is no separate registration form). Passkey
sign-in remains available for platform operators.

---

## 1. Prerequisites

- The stack is running: `just db-up` (Postgres + Odoo containers) and
  `just start` (API server on `http://localhost:8081`).
- Odoo answers at `http://localhost:8070` (database `odoo_local`; the
  first-time setup is `just odoo-local-init`, credentials `admin`/`admin`).
- The MCP Inspector is running at `http://localhost:6274`
  (`npx @modelcontextprotocol/inspector`).

Quick health checks:

```bash
curl -s -o /dev/null -w "%{http_code}\n" http://localhost:8081/health   # 200
curl -s -o /dev/null -w "%{http_code}\n" http://localhost:8070/web/login # 200
```

## 2. Create your test user in Odoo

1. Open `http://localhost:8070` and sign in as `admin` / `admin`.
2. Settings → Users & Companies → Users → **New**.
3. **The login must be an email address** (for example
   `you@systemprompt.local`). Sign-in with a non-email login is refused,
   because platform accounts key on email.
4. Under Access Rights, grant the applications the user should reach.
   Note that "Settings/Administration" alone does **not** grant application
   data access — each app (Sales/CRM, Project, …) has its own groups. For a
   full-access tester, mirror the built-in admin: Sales → Administrator,
   plus whichever apps you plan to exercise.
5. Save, then set a password via the ⚙ action menu → **Change Password**.
6. If the user has 2FA enabled, they must sign in to the platform with an
   Odoo **API key** (Preferences → Account Security → New API Key) instead
   of their password — Odoo refuses 2FA passwords over RPC.

A ready-made local admin exists on this machine:
`admin@systemprompt.local` / `sp-local-admin-2026` (all admin groups).

## 3. Connect the MCP Inspector

1. In the Inspector, choose transport **Streamable HTTP** and set the URL to
   `http://localhost:8081/api/v1/mcp/odoo/mcp`.
2. Click **Connect**. The server answers 401 and the Inspector starts OAuth:
   a browser window opens on the platform sign-in page
   (`http://localhost:8081/admin/login?client_id=…`).
3. Enter the Odoo user's **email** and **password (or API key)** and sign in.
4. The browser returns to the Inspector with an authorization code; the
   Inspector exchanges it and shows **Connected**.

What just happened on first sign-in:

- A platform user was created for you (roles `["user"]`) with a
  `federated_identities` row (issuer `odoo:http://localhost:8070/odoo_local`).
- Your Odoo credential was auto-linked (`odoo_identity`), so Odoo MCP tools
  run **as you** — records you create in Odoo carry your name.

Prefer a passkey? On the sign-in page click **"Use a passkey instead"** —
that returns to the built-in WebAuthn form (`prompt=passkey`).

## 4. Test the tools

1. In the Inspector, open the **Tools** tab and click **List Tools**.
2. Work through the tools you care about. A useful smoke sequence:
   - a read tool first (list/search — e.g. CRM leads or partners) to prove
     data access;
   - then a write tool (e.g. create a note or activity) and confirm in the
     Odoo UI that the record exists **and is attributed to your user**.
3. If a tool answers "You are not allowed to access …", that is Odoo
   enforcing *your* user's access rights — go back to step 2.4 and add the
   app's group to your Odoo user. No platform change or re-login is needed;
   permissions apply on the next call.

## 5. Verifying from the CLI

```bash
systemprompt admin users list                      # your JIT-created user
systemprompt infra logs request list --limit 10    # AI/gateway requests
systemprompt infra logs trace list --limit 10      # MCP tool calls
systemprompt infra logs view --level error --since 10m
```

## 6. General notes & troubleshooting

- **Registration = first sign-in.** Deleting the platform user and signing
  in again recreates it. Odoo's own user list is the access boundary.
- **Issuer mismatch (RFC 9207)**: the authorization response carries
  `iss=http://localhost:8081`; if your client expects a different issuer,
  its discovery URL and the server's `api_external_url` disagree.
- **"redirect_uri not registered"**: the client's callback URL must be
  registered for its `client_id` (Inspector registers itself via dynamic
  client registration; fixed clients need a row in
  `oauth_client_redirect_uris`).
- **Credential rotation**: changing the Odoo password / revoking the API key
  strands the stored copy until the next sign-in (or a profile re-link)
  refreshes it.
- **Sign-in throttling**: repeated failures block the login and source IP
  for fifteen minutes.
- Odoo connection settings live in the profile secrets
  (`odoo_url`, `odoo_db`); verify with `GET /admin/api/profile/odoo`
  (`configured: true`).
