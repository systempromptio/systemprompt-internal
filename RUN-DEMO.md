# Running the demo — accounts, logins, and who plays whom

Companion to `DEMO.md` (the script). This file is the cast list: every account the demo uses, how to
sign in with each, and how to run the whole thing as a **real** user instead of the seeded test pair.

Roles come from Odoo groups on every sign-in (`services/access-control/odoo-roles.yaml`): every Odoo
user gets `user`; membership of **Settings → Administration: Settings** (`base.group_system`) adds
`admin`. `user` sees demo steps 1–3; `admin` also gets steps 4–5 and the admin dashboards.

## The cast (local, seeded)

All local test data — never reuse these anywhere real.

| Plays | Account | Password | Platform roles | Demo steps |
|---|---|---|---|---|
| The salesperson (Act I) | `ed+notadmin@systemprompt.io` | `e2e-live-password-2026` | `user` | 1–3 |
| The admin (Act II) | `ed@systemprompt.io` | `e2e-live-password-2026` | `admin, user` | 1–5 |
| Odoo back office (proof shots) | `admin` / `admin` | — | (Odoo UI only, `http://localhost:8070`, db `odoo_local`) | show chatter in beat 3 |

Both e2e users are (re)seeded idempotently by `just e2e-live`. Verify roles after first sign-in with
`systemprompt admin users list`.

## How to log in, per surface

- **Browser (admin UI):** `http://localhost:8081/admin/login` — Odoo email + password. First sign-in
  JIT-creates the platform account; roles are recomputed from Odoo groups at every sign-in.
- **Cowork (per machine):**
  ```bash
  systemprompt-internal-bridge login <sp-live-…> --gateway http://localhost:8081
  systemprompt-internal-bridge whoami      # confirm which cast member you are
  systemprompt-internal-bridge logout      # switching actors: logout purges token cache + sync state
  ```
  Mint a PAT for a user: `systemprompt admin users api-key issue --user <email> --name demo`
  (secret prints once). Or use the browser device-link flow — the approval page shows which account it
  links and offers "Not you? Use a different account".
- **MCP Inspector / OAuth:** `http://localhost:8081/api/v1/mcp/odoo/mcp` — the OAuth dance signs you
  in as any of the users above; every tool then runs in Odoo as that user.

**Switching actors mid-demo (Act I → Act II):** `bridge logout` → `bridge login` with the admin's PAT
→ `bridge sync` → restart Cowork. Budget 2 minutes; rehearse it once.

## Running it as your real user (recommended for the live demo)

The demo is not tied to the test users — any Odoo account works, because every tool call executes in
Odoo as whoever signed in. To put yourself on stage:

**Locally:**
1. In the Odoo UI (`http://localhost:8070`, `admin`/`admin`): Settings → Users → New. Create yourself
   (e.g. `ed@systemprompt.io`) with a password, **Sales: User** access so the CRM tools have something
   to act on, and assign yourself a few leads so triage finds *your* pipeline.
2. Decide the role: leave Administration empty to play the salesperson (`user`, steps 1–3); set
   **Administration: Settings** to also be platform `admin` (steps 4–5). You can flip this in Odoo at
   any time — the new role applies at your next sign-in, promotion and demotion alike.
3. Sign in at `/admin/login` with those credentials, then bridge-login as yourself (mint a PAT:
   `systemprompt admin users api-key issue --user ed@systemprompt.io --name demo`).

The cleanest theatre is real-you in both acts: run Act I with your Administration setting empty, then
have a colleague (or the Odoo admin session) grant you **Administration: Settings**, sign in again,
and open Act II with "my role just changed in the identity system — no restart, watch the picker
change". That beat is itself the governance pitch.

**On production** (`internal.systemprompt.io`, Odoo at `odoo.systemprompt.io` once DNS lands): the
same mechanics with your real Odoo credentials — `ed@systemprompt.io` is already an Odoo
administrator there, so you carry `admin, user` and see all five skills. No credentials for
production belong in this file or anywhere in the repo: use your own Odoo password/API key, and mint
PATs on the spot with `admin users api-key issue`. For a second real cast member, any of the real
Odoo users can play the salesperson with their own login.

## Preflight (60 seconds, before the audience)

```bash
curl -s http://localhost:8081/health                      # 200
systemprompt admin users list                             # cast has the right roles
TOKEN=$(systemprompt admin session login --email <your-actor> --token-only --profile local)
curl -s -H "Authorization: Bearer $TOKEN" http://localhost:8081/v1/bridge/manifest \
  | python3 -c "import json,sys;p=json.loads(json.load(sys.stdin)['payload']);print(sorted(s['id'] for s in p['skills']))"
# salesperson: brand, demo_approval_hold, demo_blocked_tool, demo_secret_refusal,
#              lead_factsheet, send_email, systemprompt_setup
# admin: those plus demonstrate_governance, governance_readback, manage_platform,
#              show_activity, systemprompt_setup_admin, update_leads
```

**Check the approval queue is empty before you start** — a leftover parked call from a rehearsal
makes step 3b ambiguous on stage:

```bash
curl -s -H "Authorization: Bearer $TOKEN" http://localhost:8081/admin/governance/approvals >/dev/null
# or just open /admin/governance/approvals in the browser you are already screen-sharing
```

Then run the acts per `DEMO.md`.

## The one way to break this demo

**Act I must be run by a non-admin.** The `require_approval` stage carries `exempt_scopes: [admin]`,
which exempts an admin *requester* — an admin's calls are never held. So if you play the salesperson
with your own admin account (easy to do on production, where `ed@systemprompt.io` carries `admin,
user`), `note_add`, `email_send` and `channel_post` all sail through, the approvals queue stays empty,
and step 3b has nothing to approve. It looks exactly like the trust layer is switched off.

It isn't — you're exempt, and deliberately so: an admin approving their own call is a rubber stamp,
not a control, and that exemption is the thing that guarantees the approver and the requester are
different people. But the audience cannot see that, so run Act I as the salesperson every time.

The exemption is one-directional and worth saying out loud if anyone asks: it stops an admin from
being *held*, not from *approving*. Act II's admin is the approver, which is the point.

**Optional third cast member.** DEMO.md's step 3b has a race beat — a second admin clicking Deny on a
call the first has already approved, showing the first decision stands and the original approver stays
stamped. It needs a second admin account signed in on another screen. Skip the beat if you are running
the demo with two people; nothing else depends on it.

