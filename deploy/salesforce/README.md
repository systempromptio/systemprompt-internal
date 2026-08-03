# Salesforce org provisioning

The org's configuration is code. `services/salesforce/org.yaml` declares what an
org should look like, and a Rust command makes an org match it:

```bash
# authenticate (RFC 7523 JWT-bearer — no browser, no sf CLI, no Node)
export SF_TARGET_MY_DOMAIN=https://<org>.my.salesforce.com
export SF_TARGET_CONSUMER_KEY=3MVG9...
export SF_TARGET_JWT_SUBJECT=admin@example.com     # Salesforce Username, not email
export SF_TARGET_PRIVATE_KEY="$(cat provisioning.key)"

systemprompt plugins run salesforce diff
systemprompt plugins run salesforce apply --dry-run
systemprompt plugins run salesforce apply
```

`export` reads a live org into spec shape; `diff` reports drift; `apply` fixes
it. `--dry-run` submits the metadata package with `checkOnly`, so Salesforce
runs its full validation and writes nothing.

Implementation lives in `extensions/web/admin/src/services/salesforce_org/`,
next to the JWT-bearer token minting it reuses. `extensions/cli/salesforce/` is
just the argument parser.

`org-export.json` is a dated snapshot of the dev org taken 2026-07-31, kept as a
reference for what the org looked like before any of this existed.

## What is and is not automated

| Step | Headless? |
|---|---|
| Read the app, OAuth settings, policies, permission sets | yes — REST/Tooling SOQL |
| Create/update the app, its OAuth settings and policies | yes — Metadata REST deploy |
| Create permission sets and app pre-authorization grants | yes — REST sObject writes |
| Assign a permission set to a user | yes — REST sObject write, from the platform database |
| Activate a standard hosted MCP server | yes — Tooling `McpServerAccess` PATCH (API 67.0+) |
| **Create the first app in a brand-new org** | **no** — Setup only, once per org |

One manual step per org, one-time, and only because a consumer secret is not
readable through any API. Everything after it is `apply`.

## Why not the `sf` CLI

This org issues **JWT-based access tokens**. The SOAP Metadata API rejects them
outright, and that is what `sf project retrieve` and `sf project deploy` are
built on:

```
$ sf org list metadata-types -o astound-dev
Error (1): SOAP API does not support JWT-based access tokens. You must disable
the "Issue JSON Web Token (JWT)-based access tokens" setting in your Connected
App or External Client App
```

The Metadata **REST** deploy resource (`/services/data/v67.0/metadata/deployRequest`)
accepts the same tokens without complaint — verified end to end, deploy status
`Succeeded`. So the tool deploys over REST and never needs SOAP, which also
means it never needs a retrieve: the metadata XML is generated from the spec
rather than pulled from an org.

That is why there is no second "provisioning app" and no Node dependency.

## Bootstrapping a brand-new org

Only needed once, and only because a consumer secret is not readable through
any API.

1. Setup → External Client App Manager → New. Name it, set the contact email.
2. Enable OAuth. Callback URL must match `redirect_uri` in
   `services/web/config/salesforce.yaml` exactly.
3. Scopes: at minimum `api`, `refresh_token`, `openid`, and **`mcp_api`** —
   without the last, every MCP tool call fails.
4. **Use digital signatures**: upload the public certificate whose private key
   the platform will hold.
   ```bash
   openssl req -x509 -sha256 -nodes -days 730 -newkey rsa:2048 \
     -keyout provisioning.key -out provisioning.crt \
     -subj "/CN=systemprompt-provisioning"
   ```
5. Policies → Permitted Users: **Admin approved users are pre-authorized**.
6. Copy the consumer key. Store it and the private key as secrets
   (`SALESFORCE_PRIVATE_KEY`, or `salesforce_private_key` in the profile's
   `secrets.json`).

From here `apply` takes over — it will create the permission set, wire the
pre-authorization grant, and bring the OAuth settings and policies in line.

## Activating the hosted MCP server

`apply` does this. An earlier version of this document claimed no API existed;
that was wrong, and it was an artefact of pinning the API version to 64.0.

At **67.0** the Tooling API exposes `McpServerAccess`, which is the activation
record itself — `createable`, `updateable` and `deletable` all true:

| DeveloperName | MasterLabel | Active |
|---|---|---|
| `platform_sobject_all` | sobject-all | true |
| `industries_engagement_interaction` | engagement-interaction | true |

`export` reads `Active`, `diff` reports a server switched off as real drift, and
`apply` PATCHes it back on. The dead end that produced the wrong claim was
`McpServerDefinition`, which holds *custom* server definitions and is empty in an
org using a standard hosted server — it is the wrong object, not a missing API.

Apply stays additive: a server active in the org but absent from the spec is left
alone. A server named in the spec but absent from the org is an error, because
absence means the org does not offer it and activation cannot fix that.

## Fields that cannot be verified

`callback_url`, `pkce_required`, `consumer_secret_optional` and
`named_user_jwt` live on
`ExtlClntAppGlobalOauthSettings`, which is not a queryable sObject — the
`ExtlClntAppOauthSetAttr` bag that might have carried them is empty. They are
deployed on every apply but cannot be read back, so `diff` lists them under
"always applied" instead of counting them as verified. `export` carries them
forward from the committed spec rather than inventing values.

## Metadata schema

The four External Client App metadata types are not documented in a form that
matches what the API accepts, and this org cannot retrieve them. The element
names below were read back from the live org by submitting deliberately invalid
packages under `checkOnly` (which writes nothing) and reading the validation
errors, each of which names the rejected element.

| Type | Path in package | Elements |
|---|---|---|
| `ExternalClientApplication` | `externalClientApps/<name>.eca` | `contactEmail` (required), `label`, `description`, `contactPhone`, `distributionState`, `isProtected`, `logoUrl`, `infoUrl`, `iconUrl` |
| `ExtlClntAppGlobalOauthSettings` | `extlClntAppGlobalOauthSets/<name>_glbloauth.ecaGlblOauth` | `externalClientApplication` (required), `callbackUrl` (required), `consumerKey`, `certificate`, `isConsumerSecretOptional`, `isPkceRequired`, `isNamedUserJwtEnabled`, `isIntrospectAllTokens` |
| `ExtlClntAppOauthSettings` | `extlClntAppOauthSettings/<name>_oauth.ecaOauth` | `externalClientApplication`, `commaSeparatedOauthScopes`, `isFirstPartyAppEnabled`, `singleLogoutUrl`, `clientAssertionCertificate` |
| `ExtlClntAppOauthConfigurablePolicies` | `extlClntAppOauthPolicies/<name>_oauthPlcy.ecaOauthPlcy` | `permittedUsersPolicyType`, `ipRelaxationPolicyType`, `refreshTokenPolicyType`, `refreshTokenValidityPeriod`, `refreshTokenValidityUnit`, `requiredSessionLevel`, `startUrl` |

Scope tokens are their own vocabulary, distinct from the sObject column names —
`Basic` not `SSO`, `MCP` not `MCP_API`. The full set:

```
Basic, Api, Web, Full, RefreshToken, OfflineAccess, OpenID, Profile, Email,
Address, Phone, CustomPermissions, CustomApplications, Content, Lightning,
Chatter, Wave, Eclair, Pardot, Interaction, ForgotPassword, UserRegistration,
PwdlessLogin, EinsteinGPT, SFApiPlatform, SCRT, Chatbot, MCP, CDP, CDPQuery,
CDPProfile, CDPIngest, CDPSegment, CDPIdentityResolution,
CDPCalculatedInsight, DataCloudUserClaims
```

`OauthScopesHUB_API` exists on the sObject with no counterpart in that list, so
it cannot round-trip; export warns if it is ever found enabled.

Two enums Salesforce does enumerate in its errors, so they are typed:
`ipRelaxationPolicyType` is `Enforce | Bypass | Bypass_2factor |
Enforce_relaxrefresh`, and `refreshTokenValidityUnit` is `Hours | Days |
Months`. The rest (`ExtlClntAppDistState`, `PermittedUsersPolicyType`,
`RefreshTokenPolicyType`, `SessionSecurityLevel`) are carried as strings — the
parse error does not list valid values, and guessing them would be worse than
letting Salesforce reject a bad one with a clear message.

### The certificate is the sharpest edge

`certificate` is in schema on `ExtlClntAppGlobalOauthSettings`, and it is not
readable back through any query API — so a deploy cannot round-trip it and must
be *given* it. A package that omits it clears the app's digital signature, the
JWT-bearer grant fails with `invalid_grant: invalid assertion`, and the tool has
just destroyed the credential it authenticates with. It cannot fix that itself;
recovery is a manual certificate upload in Setup.

The Setup UI couples "Enable JWT Bearer Flow" to the certificate, so the visible
symptom is an unticked flow checkbox rather than a missing certificate. There is
no separate metadata element for that checkbox — probed at 67.0,
`isJwtBearerFlowEnabled` and every plausible spelling are out of schema on all
four components. The certificate *is* the setting.

`apply` therefore requires `SF_TARGET_CERTIFICATE` and refuses to build a package
without it. That refusal is the guard; the emitted element is the fix.

### The version bump hazard

A metadata deploy is **declarative**. An element that is in schema at the
deployed version and omitted from the package takes its default — it is not left
alone. So raising `<version>` can silently rewrite settings that the previous
version could not even name.

`isNamedUserJwtEnabled` is the worked example. It is rejected at 64.0 as "not
valid in version 64.0" and accepted at 67.0. It controls whether the org issues
JWT-format access tokens, which the REST metadata deploy accepts and the SOAP
Metadata API rejects — so at 67.0 a package omitting it could disable the very
mechanism it was deployed with. It is now declared explicitly in `org.yaml`
(`oauth.named_user_jwt`) and always emitted.

Re-probed at 67.0, these elements also came into scope on
`ExtlClntAppGlobalOauthSettings` and are **not** yet declared in the spec:
`isDeviceFlowEnabled`, `isIntrospectAllTokens`, `isSecretRequiredForRefreshToken`,
`isSecretRequiredForTokenExchange`, `shouldRotateConsumerKey`. None is readable
back through any query API, so their current values are unknown and an apply may
be resetting them to defaults. Nothing in use depends on them today; declare any
one of them explicitly before relying on it.

The probe method, which is the only way to establish this: submit a package at
the target `<version>` with one candidate element added, under `checkOnly`, and
read the result. `Succeeded` means in schema; `Element {ns}name invalid at this
location in type X` means out of schema; a value-validation complaint also means
in schema. Re-derive the table above this way on every version bump — do not
hand-edit it.
