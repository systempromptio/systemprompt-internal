# Setting up a new Salesforce org

How to stand up a fresh Salesforce org, point this repository at it, and get a
user signing in through it — end to end.

`org.yaml` in this directory is the desired state. This file is the runbook for
getting an org to the point where that spec can be applied.

Companion doc: `deploy/salesforce/README.md` covers *why* the tooling works the
way it does — which API surfaces are writable, why the `sf` CLI is not used, and
the metadata schema. Read that if something here fails and you need to reason
about it rather than follow it.

## The shape of the job

One step genuinely cannot be automated, one-time per org: creating the first
External Client App, because its consumer secret is not readable through any
Salesforce API.

Everything after it is `systemprompt plugins run salesforce apply` — including
assigning the permission set and activating the hosted MCP server, both of which
earlier versions of this runbook did by hand.

---

## 1. Create the org and a signing key pair

Sign up for a Developer Edition org at <https://developer.salesforce.com/signup>.

Note the My Domain URL from **Setup → My Domain → Current My Domain URL**. It
looks like `https://orgfarm-xxxxxxxx-dev-ed.develop.my.salesforce.com`.

Generate the key pair used for the RFC 7523 JWT-bearer grant. The platform holds
the private key; Salesforce holds the certificate.

```bash
openssl req -x509 -sha256 -nodes -days 730 -newkey rsa:2048 \
  -keyout salesforce.key -out salesforce.crt \
  -subj "/CN=systemprompt-internal"
```

Keep `salesforce.key` out of the repository. It goes in the profile's
`secrets.json` or an environment variable, never in `services/`.

## 2. Create the External Client App

**Setup → External Client App Manager → New Connected App → Create External
Client App.**

| Field | Value |
|---|---|
| Name | `Systemprompt_SSO` — must match `developer_name` in `org.yaml` |
| Contact email | your address |
| Distribution state | Local |
| Enable OAuth | yes |
| Callback URL | `https://<your-host>/admin/auth/salesforce/callback` |
| Scopes | `api`, `refresh_token`, `openid`, **`mcp_api`** |
| Use digital signatures | upload `salesforce.crt` |
| Permitted Users | **All users may self-authorize** — `apply` tightens this |

Without `mcp_api` every MCP tool call fails. Without the certificate the
JWT-bearer grant cannot mint a token, so `apply` cannot run at all.

Copy the **Consumer Key** and **Consumer Secret** before leaving the page. The
secret is not retrievable through any API — this is the reason this step is
manual.

## 3. Point the repository at the org

**`services/web/config/salesforce.yaml`** — the SSO client:

```yaml
enabled: true
my_domain: "https://<your-org>.my.salesforce.com"
client_id: "<consumer key>"
redirect_uri: "https://<your-host>/admin/auth/salesforce/callback"
allowed_email_domains:
  - yourcompany.com
```

**`services/salesforce/org.yaml`** — set `external_client_app.oauth.callback_url`
to the same value as `redirect_uri`. Salesforce compares them character for
character; a trailing slash difference fails the login with no useful error.

**`services/access-control/plans.yaml`** — claim the email domain:

```yaml
organizations:
  - slug: yourcompany
    name: Your Company
    plan: enterprise
    email_domains:
      - yourcompany.com
```

Skipping this does not fail loudly. The user signs in, lands unattached, gets no
organization, therefore no plan grants and no seat check — they simply see
nothing. Note the two independent domain lists: `allowed_email_domains` decides
**who may sign in at all**, `email_domains` decides **which organization they
join**. Keep them in sync.

**Secrets** — `.systemprompt/profiles/local/secrets.json`, or the equivalent
environment variables (`SALESFORCE_CLIENT_SECRET`, `SALESFORCE_PRIVATE_KEY`):

```json
{
  "salesforce_client_secret": "<consumer secret>",
  "salesforce_private_key": "-----BEGIN PRIVATE KEY-----\n...\n-----END PRIVATE KEY-----",
  "salesforce_certificate": "-----BEGIN CERTIFICATE-----\n...\n-----END CERTIFICATE-----"
}
```

`salesforce_certificate` is the public half of the same key pair — the
`salesforce.crt` uploaded in step 2. It is not secret; it lives here so the pair
stays together, and because **`apply` refuses to run without it** (see step 4).
Lose it and it can be regenerated from the private key without rotating
anything:

```bash
openssl req -x509 -new -sha256 -nodes -days 730 \
  -key salesforce.key -out salesforce.crt -subj "/CN=systemprompt-internal"
```

## 4. Apply the spec

```bash
export SF_TARGET_MY_DOMAIN="https://<your-org>.my.salesforce.com"
export SF_TARGET_CONSUMER_KEY="<consumer key>"
export SF_TARGET_JWT_SUBJECT="you@yourcompany.com"
export SF_TARGET_PRIVATE_KEY="$(cat salesforce.key)"
# Only when targeting an org other than this deployment's own — otherwise the
# certificate comes from SALESFORCE_CERTIFICATE or the profile's secrets.json.
export SF_TARGET_CERTIFICATE="$(cat salesforce.crt)"

systemprompt plugins run salesforce diff              # what differs
systemprompt plugins run salesforce apply --dry-run   # full validation, writes nothing
systemprompt plugins run salesforce apply             # apply it
```

The certificate paired with the private key — the same `salesforce.crt` uploaded
in step 2 — must be resolvable, from `SF_TARGET_CERTIFICATE`,
`SALESFORCE_CERTIFICATE`, or `salesforce_certificate` in the profile's
`secrets.json` (step 3 stores it there, so targeting this deployment's own org
needs nothing extra). **Apply refuses to run without it**, deliberately. A metadata deploy is declarative, so a
package that omits `<certificate>` clears the app's digital signature; the
JWT-bearer grant then fails with `invalid_grant: invalid assertion`, and because
that grant is how this tool authenticates, it cannot repair the damage itself.
Recovery costs a manual certificate upload in Setup. `export` and `diff` do not
need it.

`SF_TARGET_JWT_SUBJECT` is the Salesforce **Username**, not the email address.
The two routinely differ (`you@company.com.dev`, `ed.aa5967144c6c@agentforce.com`),
and Salesforce matches the assertion `sub` on the Username. Find it under
**Setup → Users**.

`apply` sets the OAuth scopes, policies, callback URL and PKCE requirement,
creates the `Salesforce_MCP_Access` permission set together with the
`SetupEntityAccess` grant that pre-authorizes the app, assigns that permission
set to every user, and activates the hosted MCP servers the spec names.

`--dry-run` submits a real metadata package with `checkOnly`, so Salesforce runs
its full validation and writes nothing. Use it first every time.

### Who gets assigned

Assignees come from the platform database, not from `org.yaml`: the
`salesforce_user_identities` table records a Salesforce Username for every user
who has completed an SSO login. No personal data goes in the repository, and no
list has to be kept in sync by hand.

A brand-new org has no logins yet, so name yourself explicitly the first time:

```bash
systemprompt plugins run salesforce apply --user "you@yourcompany.com.dev"
```

That value is the Salesforce **Username**, the same thing as
`SF_TARGET_JWT_SUBJECT`. Skipping it on a fresh org is safe but pointless — the
apply will configure the org and assign nobody.

If the platform database is unreachable, `apply` reports which assignments it
skipped and carries on rather than failing; the metadata half is independently
useful. Re-run it once the database is back.

### Ordering

`apply` creates the permission set, the grant and the assignments **before** it
deploys the metadata that flips Permitted Users to `AdminApprovedPreAuthorized`.
That ordering is deliberate: from the moment the policy changes, only holders of
the permission set can mint a token. Doing the deploy first — which is what this
tool used to do — locked out everyone not already assigned, including the
operator running the command.

## 5. Restart and sign in

```bash
just build && just start
```

A restart is required, not just `just publish`: `salesforce.yaml` is cached in a
`OnceLock` at first read, and `plans.yaml` is projected into access-control rules
by `publish_pipeline` at startup.

Then visit `/admin/auth/salesforce/start`. The first login auto-creates the local
account, provided the email domain is allow-listed, a seat is free, and the user
is pre-authorized on the app.

## Verify

```bash
# the org matches the spec on every readable field
systemprompt plugins run salesforce diff --exit-code   # 0 = clean, 1 = drift

# the per-user Salesforce bearer mints
curl -H "Authorization: Bearer <user jwt>" \
     http://localhost:8080/api/public/salesforce/token
```

Then sign in as a real user and confirm three things: the login succeeds, the
user appears in the expected organization, and the Salesforce tools are visible
to them.

## Troubleshooting

| Symptom | Cause |
|---|---|
| `apply` cannot authenticate | Certificate not uploaded, or `SF_TARGET_JWT_SUBJECT` is the email rather than the Username |
| `invalid_grant: invalid assertion` after an apply | The deploy cleared the app's certificate — an apply that predates the `SF_TARGET_CERTIFICATE` requirement. Re-upload a certificate matching the private key in Setup; the "Enable JWT Bearer Flow" checkbox is coupled to it and unticks when the certificate goes, so tick it again on the way through |
| `invalid_app_access: user is not admin approved` | The deploy dropped the `SetupEntityAccess` grant. Add the permission set on the app's Policies tab, or re-run `apply` |
| Authentication worked, then stopped | An org configured before assignment was automated: the policy flipped to `AdminApprovedPreAuthorized` while nobody held the permission set. Set Permitted Users back to "All users may self-authorize" in Setup, then re-run `apply` — the current ordering assigns first and cannot reproduce it |
| Login redirects with `?sso=not_provisioned` | `auto_provision: false` and no account exists |
| Login redirects with `?sso=seat_limit` | The organization is full |
| User signs in but sees no Salesforce tools | Their domain is not in any organization's `email_domains` — they landed unattached |
| Seat limits never trigger | Same cause. No organization means no seat check |
| MCP tool calls fail with 401 | `mcp_api` scope missing, or the hosted server is not Active — `diff` reports the second as drift |
| Callback fails with no useful error | `callback_url` and `redirect_uri` differ — Salesforce compares them exactly |

## What `diff` will not tell you

`callback_url`, `pkce_required`, `consumer_secret_optional` and `named_user_jwt`
live on
`ExtlClntAppGlobalOauthSettings`, which is not a queryable sObject. They are
deployed on every `apply` but cannot be read back, so `diff` lists them under
"always applied" rather than claiming to have verified them. If a callback URL is
wrong, `diff` will report a clean org.
