---
title: "Step 2 — JWT-Bearer and the Signing Certificate"
description: "Generate a signing keypair, upload the certificate to your Salesforce app, pre-authorize users, and provision the private key so the platform can mint per-user Salesforce tokens on demand."
author: "Astound Digital"
slug: "salesforce-jwt-bearer"
keywords: "salesforce, jwt bearer, rfc 7523, digital signatures, certificate, private key, preferred_username, pre-authorized, openssl"
kind: "guide"
public: true
tags: ["salesforce", "jwt", "security", "setup"]
published_at: "2026-07-29"
updated_at: "2026-07-31"
after_reading_this:
  - "Generate an RSA keypair and upload the certificate to your Salesforce app"
  - "Pre-authorize the profiles or permission sets that may use the integration"
  - "Provision SALESFORCE_PRIVATE_KEY safely"
  - "Understand why the Salesforce Username, not the email, is what gets signed"
related_playbooks:
  - title: "Start Here — Standing Up the Gateway"
    url: "/documentation/use-case-admin"
  - title: "Overview"
    url: "/documentation/salesforce"
  - title: "Step 1 — Salesforce App Setup"
    url: "/documentation/salesforce-app-setup"
  - title: "Step 3 — Hosted MCP Access"
    url: "/documentation/salesforce-hosted-mcp"
---

# Step 2 — JWT-Bearer and the Signing Certificate

**TL;DR:** Generate an RSA keypair, upload the certificate to your Salesforce
app with digital signatures enabled, pre-authorize the users who may use the
integration, and provide the private key as `SALESFORCE_PRIVATE_KEY`. This is
what lets the platform mint a fresh Salesforce token for a specific user at the
moment a tool is called.

## Why This Step Exists

Step 1 got users logged in. It did not give the platform any way to *act* as
them against Salesforce afterwards.

The obvious approach — bank each user's OAuth refresh token at login and use it
later — was deliberately rejected. It means storing a long-lived, high-value
credential per user, which is exactly the kind of store that turns one breach
into an org-wide compromise.

Instead the platform uses the RFC 7523 JWT-bearer grant. At the moment a tool
call needs Salesforce access, the platform builds a short-lived assertion,
signs it with a private key that never leaves the server, and exchanges it for a
fresh access token scoped to one user. **No Salesforce tokens are stored
anywhere.** Every call mints its own, and it expires almost immediately.

The trust anchor is the certificate you upload in this step: Salesforce accepts
the assertion because it can verify the signature against a certificate you, the
administrator, explicitly installed.

## 1. Generate a Keypair

On a trusted machine:

```bash
# Private key — never leaves your secret store
openssl genrsa -out salesforce-jwt.key 2048

# Self-signed certificate — this is what you upload to Salesforce
openssl req -new -x509 -key salesforce-jwt.key -out salesforce-jwt.crt \
  -days 3650 -subj "/CN=astound-bridge"
```

A self-signed certificate is correct here. Salesforce is not validating a chain
of trust — it is pinning the exact certificate an administrator installed on the
app. A long expiry avoids an outage when it lapses, but note the date: when the
certificate expires, every mint fails at once and the fix requires Salesforce
Setup access. Put the expiry in a calendar.

Treat `salesforce-jwt.key` as the most sensitive artifact in this integration.
Anyone holding it can mint a Salesforce token as any pre-authorized user in your
org, without a password and without triggering MFA.

## 2. Upload the Certificate

In Salesforce Setup, open the app from step 1 and edit its OAuth settings:

1. Enable **Use digital signatures**.
2. Upload `salesforce-jwt.crt`.
3. Save.

Salesforce may take several minutes to propagate the certificate.

Depending on your org's release, the app may also expose a setting controlling
whether it issues JWT-based access tokens. Salesforce's exact wording for this
toggle varies by release and we have not verified a single canonical label, so
check your app's OAuth settings for an option of that description and enable it
if present. If mints fail with an otherwise unexplained error after the
certificate is correctly installed, this setting is the first thing to check.

## 3. Pre-Authorize Users

This is the step most often missed, and it fails in a confusing way.

Set the app's permitted users to **Admin approved users are pre-authorized**,
then assign the profiles or permission sets whose members may use the
integration.

The consequence is worth stating plainly: **a user who is not pre-authorized
can still log in successfully via SSO but every Salesforce tool call will
fail.** Step 1's login path and this step's mint path are authorized
independently. A user in that state has a working platform account and a
completely broken Salesforce integration, which looks like a platform bug and
is not one.

Pre-authorization is also your access control boundary. Only users on an
assigned profile or permission set can be acted for at all, regardless of what
the platform's own roles allow.

## 4. Provision the Private Key

The platform reads the PEM from the `SALESFORCE_PRIVATE_KEY` environment
variable first, then falls back to the `salesforce_private_key` key in the
active profile's gitignored `secrets.json`. It is never stored in
`salesforce.yaml`.

```bash
export SALESFORCE_PRIVATE_KEY="$(cat salesforce-jwt.key)"
```

The value must be the full PEM including the `-----BEGIN`/`-----END` lines and
its newlines. A key mangled into a single line will not parse, and the resulting
error names an invalid `SALESFORCE_PRIVATE_KEY` — if you see that, suspect
newline handling in whatever injected the variable before suspecting the key.

## What Gets Signed

Each mint builds these claims and signs them RS256:

| Claim | Value |
|-------|-------|
| `iss` | Your app's consumer key |
| `sub` | The user's Salesforce **Username** |
| `aud` | Your org's My Domain base URL |
| `exp` | 180 seconds from now |

The assertion is POSTed to `{my_domain}/services/oauth2/token` with
`grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer`, and Salesforce returns
an access token plus the instance URL it is scoped to.

The 180-second lifetime is well inside Salesforce's five-minute ceiling. It
bounds replay of the assertion itself: even if one were captured in transit, it
is worthless within three minutes.

## The Username Trap

`sub` must be the Salesforce **Username** — not the login email. In most orgs
these differ. A Salesforce Username often looks like
`jane.smith@company.com.prod` or `jane.aa5967144c6c@agentforce.com`, while the
user's email is plainly `jane.smith@company.com`.

The platform handles this by capturing the `preferred_username` claim at SSO
login and storing it against the user, so later mints sign the right value. This
is why the `profile` scope is mandatory in step 1 — without it there is no
`preferred_username` claim to capture.

**The failure mode to know about:** if no Salesforce Username was ever recorded
for a user, the platform falls back to signing with their login email. That is
only correct in orgs where the email *is* the Username. Everywhere else the mint
fails, and the fallback is logged as a warning rather than surfaced as an error.
It affects any user who has a platform account but has not completed a
Salesforce login — for example, someone created by an admin who then went
straight to the bridge.

The fix is always the same: have the user complete one **Sign in with
Salesforce** login. That captures the Username and every subsequent mint works.

## Verify

The token accessor endpoint is the mint path, so calling it directly proves the
whole step:

```bash
# As a signed-in user, with the access_token cookie's JWT
curl -s https://<your-host>/api/public/salesforce/token \
  -H "Authorization: Bearer <your platform JWT>"
```

A working setup returns:

```json
{ "access_token": "00D...", "instance_url": "https://yourcompany.my.salesforce.com" }
```

A `502` means Salesforce refused the assertion — the status deliberately blames
upstream rather than reporting a platform fault. Check the server logs for the
response body Salesforce returned; it names the actual reason.

## Troubleshooting

| Symptom | Likely cause |
|---------|--------------|
| `502` with `invalid_grant` from Salesforce | The user is not pre-authorized, or the `sub` is not a valid Username in this org |
| `502` naming a missing private key | `SALESFORCE_PRIVATE_KEY` is unset in the server's environment, and no `salesforce_private_key` exists in `secrets.json` |
| An error naming an invalid `SALESFORCE_PRIVATE_KEY` | The PEM is malformed — usually flattened newlines, or a key in an unexpected format |
| Works for some users, fails for others | The failing users are not on a pre-authorized profile, or never completed a Salesforce login so their Username was never captured |
| Worked yesterday, fails for everyone today | The certificate expired, or was replaced on the app |
| `Salesforce not configured` | `enabled` is false or the config is incomplete — see step 1 |

The "works for some users" case is the one that wastes the most time. Before
investigating anything else, confirm the affected user is on a pre-authorized
profile and has completed at least one Salesforce login.

Next: **[Step 3 — Hosted MCP Access](/documentation/salesforce-hosted-mcp)**.
