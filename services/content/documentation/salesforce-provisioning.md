---
title: "Step 4 — Users, Seats and Roles"
description: "Claim your email domains, assign a plan and seat limit, and control what a provisioned Salesforce user can reach — so SSO arrivals land in the right organization with the right entitlements."
author: "Astound Digital"
slug: "salesforce-provisioning"
keywords: "provisioning, jit, seats, seat limit, plans, organizations, access control, roles, email domains, auto_provision"
kind: "guide"
public: true
tags: ["salesforce", "provisioning", "access-control", "setup"]
published_at: "2026-07-29"
updated_at: "2026-07-29"
after_reading_this:
  - "Claim your email domains so SSO arrivals join an organization and consume a seat"
  - "Choose a plan and set a seat limit that is actually enforced"
  - "Control which plugins, agents, and MCP servers a default user can reach"
  - "Offboard a user without destroying their audit trail"
related_playbooks:
  - title: "Overview"
    url: "/documentation/salesforce"
  - title: "Step 1 — Salesforce App Setup"
    url: "/documentation/salesforce-app-setup"
  - title: "Step 5 — Rolling Out the Bridge"
    url: "/documentation/salesforce-bridge-rollout"
---

# Step 4 — Users, Seats and Roles

**TL;DR:** Claim your email domains in `services/access-control/plans.yaml` so
SSO arrivals join an organization, consume a seat, and inherit that plan's
entitlements. **Skipping this leaves seat limits entirely unenforced.**

## The Most Important Warning on This Page

A seat check runs **only** for users whose email domain is claimed by an
organization. An arrival on an unclaimed domain lands **unattached**: they get
an account, but no organization, therefore no plan grants, and — critically —
**no seat check at all**. The seat-limit machinery is real and correct, but it
simply does not engage for them.

Two organizations ship claimed, both on the `enterprise` plan:

| Slug | Name | Domains |
|------|------|---------|
| `astound-digital` | Astound Digital | `astounddigital.com`, `astoundcommerce.com` |
| `systemprompt` | systemprompt.io | `systemprompt.io` |

Every other domain is unclaimed. If you do nothing else on this page, claim
your own.

## How Just-in-Time Provisioning Works

When a verified, allow-listed Salesforce identity arrives, the platform resolves
it in a fixed order and stops at the first match:

1. **Existing mapping** — the issuer and external subject already point at a
   user. This is a returning login. The last-seen timestamp is bumped.
2. **Email link** — an active local account already owns this email address. The
   federated identity is attached to it rather than minting a duplicate. This is
   the account *merge*, and it is why the verified-email gate in step 1 is not
   optional: linking an unverified address would let a hostile identity provider
   claim an existing account.
3. **Create** — no mapping, no local account. A new user is provisioned.

On the create path, the seat check runs **before** the user row is inserted, so
a full plan rejects the login rather than creating an orphaned account that
cannot reach anything. New users are created active, email-verified, with the
`user` role, and — when their domain is claimed — a membership row in that
organization. All in one transaction.

The seat limit is enforced at both doors that can mint a seat: an admin creating
a user, and SSO just-in-time provisioning. A limit checked at only one door is
not a limit, and SSO is the door your users actually arrive through.

## 1. Claim Your Domains

Edit `services/access-control/plans.yaml`:

```yaml
organizations:
  - slug: yourcompany
    name: Your Company
    plan: enterprise
    email_domains:
      - yourcompany.com
      - yourcompany.co.uk
```

`slug` is immutable — it is the value every access rule is written against, so
renaming it later orphans those rules. `email_domains` is what binds an SSO
arrival to this organization.

An arrival whose domain matches nothing is not an error. It simply lands
unattached: no organization, no plan grants, no seat consumed. That is a
reasonable default for a stray login and a dangerous one for your whole user
base — which is precisely the trap above.

Note the two independent domain lists. `allowed_email_domains` in
`salesforce.yaml` decides **who may sign in at all**. `email_domains` here
decides **which organization they join**. A domain in the first but not the
second produces exactly the unattached-user problem. Keep them in sync.

## 2. Choose a Plan

Three plans ship, and a plan is simply a named bundle of grants plus limits:

| Plan | Seats | Monthly cap | Grants |
|------|-------|-------------|--------|
| `house` | unlimited | none | `enterprise-demo` + `astound-admin` |
| `standard` | 25 | $2,500 | `enterprise-demo` |
| `enterprise` | 500 | $50,000 | `enterprise-demo` + `astound-admin` |

`enterprise-demo` is the marketplace carrying the Salesforce plugins, agents,
and the `salesforce` MCP server. `astound-admin` is the admin control plane.

Per-customer negotiated seat counts use `seat_limit_override` on the
organization rather than editing the shared plan:

```yaml
  - slug: yourcompany
    name: Your Company
    plan: standard
    seat_limit_override: 40
    email_domains: [yourcompany.com]
```

This file is the **bootstrap** source of truth. On startup the publish pipeline
upserts each plan and organization, then projects every plan's grants into
access-control rules keyed to each organization on that plan. Dashboard edits
write to the database **only** — there is no write-back to this file. To make a
change permanent across deployments, edit the file in the repository and
redeploy.

## 3. Understand How Access Resolves

Rules are evaluated across four subject dimensions in precedence order:

```
organization (300) → role (200) → department (100) → user (0)
```

Deny overrides allow, and the narrowest scope wins. The practical consequence is
worth internalising: a customer admin can **narrow** what their plan granted, and
can never **widen** it. Entitlement is a ceiling set by the plan; local
administration operates underneath it.

Model tiering rides the same machinery through gateway-route grants. A route the
organization has no allow rule for is not dispatchable by its members — "Standard
does not get the top model tier" is enforced at the gateway rather than hidden in
a UI.

## 4. Review What a Default User Gets

`services/access-control/roles.yaml` sets the role-level rules. As shipped, a
plain `user` role gets:

- The `enterprise-demo` marketplace, included by default.
- Every Salesforce specialist agent — pipeline, account, contact, lead, case,
  activity, and the general Salesforce agent.
- The `salesforce` MCP server.

Skills, plugins, and servers inside `enterprise-demo` need no rule of their own:
they inherit the marketplace grant through the parent cascade. Add an
entity-level rule only to **deny** a specific member — a deny overrides the
inherited allow.

The admin control plane is separately gated. The `systemprompt` MCP server and
the `astound-admin` plugin are admin-only, so a normal user never sees them.
Declaring any rule for an entity opts it out of the marketplace cascade, which
is how those stay restricted.

To promote someone:

```bash
systemprompt admin users role promote <email> admin
```

The role is baked into the JWT at issue time, so the user must sign out and back
in before the change takes effect.

## 5. Decide Your Provisioning Posture

`auto_provision` in `salesforce.yaml` is the choice between two operating models:

| | `auto_provision: true` | `auto_provision: false` |
|---|---|---|
| First login | Creates the account | Rejected with `?sso=not_provisioned` |
| Admin work per user | None | Pre-create every account |
| Control point | Domain allow-list, seat limit, and Salesforce pre-authorization | Explicit per-user approval |

The frictionless path is not the loose path, provided step 4 is done properly.
Three independent gates already constrain it: the email domain must be
allow-listed, a seat must be free, and the user must be pre-authorized on the
Salesforce app. Leaving `auto_provision: true` with claimed domains and a real
seat limit is the recommended posture for most enterprises. Set it to `false`
when you need a named approval for each individual.

## Offboarding

Suspending a user frees their seat while preserving every audit row attached to
them — a seat counts only *active* members. Do not delete users to reclaim
seats; deletion destroys the governance trail that makes the integration
defensible.

Suspending an entire organization revokes every grant at once without touching a
single rule row: a non-active organization reports no organization for its
members, so its rules simply stop matching.

## Verify

```bash
# Confirm plans and organizations were projected on startup
systemprompt infra logs view --level error --since 10m
```

Then run the real test. Sign in as a Salesforce user whose domain you just
claimed, and confirm three things:

1. The login succeeds.
2. The user appears as a member of the expected organization with the expected
   seat count consumed.
3. The Salesforce tools are visible to them in the bridge.

To confirm the seat limit actually engages, temporarily set
`seat_limit_override` to the current seat count and attempt one more login. It
should be rejected with `?sso=seat_limit`. If it succeeds, the user's domain is
not claimed — go back to section 1.

## Troubleshooting

| Symptom | Cause |
|---------|-------|
| Users sign in but see no Salesforce tools | They landed unattached — their domain is not in any organization's `email_domains` |
| Seat limits never trigger | Same cause. No organization means no seat check |
| `?sso=not_provisioned` | `auto_provision: false` and no account exists |
| `?sso=seat_limit` | The organization is full. Suspend an inactive user or raise `seat_limit_override` |
| A promoted admin still sees no admin tools | The role is minted into the JWT at issue time — sign out and back in |
| Dashboard changes vanish after a redeploy | Expected. The dashboard writes only to the database; permanent changes go in the YAML |

Next: **[Step 5 — Rolling Out the Bridge](/documentation/salesforce-bridge-rollout)**.
