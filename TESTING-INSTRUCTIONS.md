# Testing the login + bridge flow (local machine)

Local-only test credentials for this development machine. Nothing here is a
production secret.

## Test accounts

| Account | Password | Where it lives | Platform role after sign-in |
|---|---|---|---|
| `e2e-admin@systemprompt.local` | `e2e-live-password-2026` | local Odoo (`odoo_local`), Settings/Administration group | `admin, user` |
| `e2e-sales@systemprompt.local` | `e2e-live-password-2026` | local Odoo, Internal User + Sales/salesman group | `user` |
| `admin` / `admin@systemprompt.local` | `admin` (Odoo UI) / `sp-local-admin-2026` (platform, this machine only) | local Odoo administrator | `admin, user` |

The two `e2e-*` users are seeded idempotently by `just e2e-live`; re-running it
recreates them and their demo CRM lead. Roles come from
`services/access-control/odoo-roles.yaml` — change a user's Odoo groups and
their platform role follows at their next sign-in.

## The login flow (what happens, in order)

1. **Browser** opens `http://localhost:8081/admin/login` (the bridge opens it
   for you during device-link).
2. Enter an **Odoo email + password** (or API key if the Odoo user has 2FA).
   The server proves the credential against Odoo (`common.authenticate`),
   JIT-creates the platform account on first sign-in, reads the user's Odoo
   groups, and maps them to platform roles.
3. A session cookie lands in the browser. Passkey / operator sign-in remains
   available from the same page for non-Odoo platform operators.

## Linking a computer through the bridge

1. In the bridge app click **Sign in** (or CLI:
   `systemprompt-internal-bridge login --gateway http://localhost:8081` with a
   PAT, or the browser device-link flow).
2. The browser opens the **Link this computer** page. It shows the account it
   is about to link — whatever session the browser already holds.
3. **Wrong account? Click "Not you? Use a different account".** That signs the
   browser session out and returns you to the login page with the link flow
   preserved; sign in as any account above and approve as that user.
4. Approve. The bridge receives a one-time code, exchanges it for a PAT, and
   syncs. `systemprompt-internal-bridge whoami` confirms the identity.

## Switching users (the two-role test)

```text
bridge logout                # purges cached token + sync state
bridge sign in → browser → "Use a different account" → e2e-admin@… → approve
bridge sync                  # admin manifest: admin skills + dashboards
bridge logout
bridge sign in → browser → "Use a different account" → e2e-sales@… → approve
bridge sync                  # salesperson manifest: no admin surface
```

PATs for headless CLI testing:
`systemprompt admin users api-key issue --user <email> --name test` (secret
prints once).

## Odoo itself

- UI: `http://localhost:8070`, database `odoo_local`, admin login
  `admin` / `admin`.
- 2FA users must use an Odoo API key (Preferences → Account Security) instead
  of their password when signing in to the platform.

## Automated equivalents

- `just e2e` — full in-process suite (roles, manifests, MCP wire, artifacts).
- `just e2e-live` — the same journey against the running stack, and the thing
  that (re)seeds the accounts above.
