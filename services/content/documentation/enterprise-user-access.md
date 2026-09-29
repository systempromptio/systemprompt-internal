---
title: "User & Access Management"
description: "Manage enterprise users from the admin UI: search, roles, session and PAT revocation, closed registration, CLI-provisioned passkey accounts, and time-bound access."
author: "systemprompt.io"
slug: "enterprise-user-access"
keywords: "users, access, roles, passkey, pat, sessions, registration, deprovisioning, expiry"
kind: "guide"
public: true
tags: ["enterprise", "admin", "access"]
published_at: "2026-08-25"
updated_at: "2026-09-16"
after_reading_this:
  - "Manage users end to end from /admin/users without touching the database"
  - "Provision users from the CLI and enrol their passkey"
  - "Understand the closed browser registration and recovery paths"
  - "Revoke sessions and personal access tokens for any user immediately"
  - "Deprovision a person immediately and bound access in time"
related_docs:
  - title: "Access Control: Who Reaches What, and Why"
    url: "/documentation/access-control"
  - title: "Authentication"
    url: "/documentation/authentication"
  - title: "Enterprise Roadmap & Known Limitations"
    url: "/documentation/enterprise-roadmap"
---

# User & Access Management

The admin UI at `/admin/users` manages accounts, roles, status, sessions, and personal access tokens. Accounts are created from the CLI and sign in with a passkey; there is no public registration page. See [Authentication](/documentation/authentication) for the sign-in model.

## Managing users from the admin UI

Open `/admin/users` as an admin. From this one page you can:

- **Search and list** users by name or email, with pagination.
- **Create** a user directly.
- **Edit roles and status** — promote or demote roles (`user`, `developer`, `knowledge_worker`, `project_manager`, `admin`), activate or suspend an account.
- **Disable or delete** an account. Disabling keeps history; deleting removes the account.
- **Revoke sessions** — force sign-out everywhere with one action.
- **Revoke personal access tokens** — every PAT the user holds can be revoked from their detail page, or from `/admin/devices/pats` which lists tokens across the instance.
- **See last activity** — each user row shows last-active time, so stale accounts are visible at a glance.

No CLI or database access is required for any of these operations. The CLI equivalent for scripting exists under `systemprompt admin users` (see `systemprompt admin users --help`).

## Provisioning and deprovisioning

There is no self-service sign-up. An operator creates the account with
`systemprompt admin users create`, grants a role if it needs more than `user`, and
mints a one-shot passkey setup link with
`systemprompt admin users webauthn generate-setup-token`. The person opens the link,
creates a passkey, and signs in with it from then on. A lost passkey is recovered the
same way: another operator mints a fresh setup link. The full procedure is in
[Authentication](/documentation/authentication).

Roles are read from the user record on every request, so a promotion or demotion
takes effect on the person's next request. To remove someone, disable or delete the
account and revoke their sessions and personal access tokens from their detail page;
revocation takes effect on the next request.

SCIM provisioning is not offered. See the [Enterprise Roadmap](/documentation/enterprise-roadmap).

## The project-manager role

`project_manager` is a read-only admin. It reaches every page of the dashboard — users, access, analytics, conversations, traces, reports, the catalog — across every project rather than one, and it changes nothing: user and role edits, access-control rules, gateway configuration and device enrolment all stay `admin`-only. It is also outside the admin control plane, so a project manager is not granted the `systemprompt` MCP server, the `systemprompt-admin` plugin or the admin skills, and an admin tool call from one is denied by governance like any other non-admin request.

The role is granted by an admin from the person's **Roles** section, like any other role. Project membership is independent of this role; users are not restricted to exactly one project.

## Personal access tokens and devices

`/admin/devices` lists every personal access token on the instance: owner, prefix (so a leaked token can be identified without exposing it), expiry, and creation time. Any token can be revoked immediately, and revocation takes effect on the next request. **Issue a token** on the same page mints one for the signed-in account and shows the secret exactly once.

A token can carry an expiry, set when it is issued from the console form or `POST /admin/devices/pats`; revoke any token immediately from the same page.

Device certificates can be given an expiry from the certificates tab (**Set expiry**). The window is recorded beside the certificate and the hourly sweep revokes the certificate once it passes.

## Time-bound sharing

Share tokens honour an expiry timestamp, and invites, setup tokens, JWTs and personal access tokens are all time-bound, so no standing credential lives forever by default.

Access itself is time-bound too. A group membership, a project membership, a manual role grant and an access-control rule each carry an optional expiry, set from the console with a date picker — the **Add member** dialog on a group or project, the **Roles** section of a person's page (one window for every role granted by hand), and the rule editor's reason box on a group's or person's Access tab. Membership tables and the `/admin/access-control` ledger show an **Expires** column, badge anything that lapses inside seven days, and the ledger's *Expiring in 7 days* filter lists what needs renewing.

How an expiry binds differs by what it is on, because two of the four are decided by code this extension does not own:

| What expires | Takes effect | How |
|---|---|---|
| Group or project membership | Immediately | Every membership read, including the `group`/`project` authorization dimensions and the `user_groups` view, hides a row outside its window. The hourly `access_expiry` sweep then stamps the row `revoked_at` so it remains as the audit trail, and recomputes the person's primary scope. |
| Manual role | Within the hour | Roles are enforced from the effective set on the user record, which the sweep rewrites without the expired grant. If that removes a manage role, the sweep revokes the person's live sessions, tokens and certificates — the same teardown a demotion in the role editor performs. |
| Access-control rule | Within the hour | Core's resolver reads the rule table directly, so the sweep deletes the expired rule. The window round-trips through `rules.yaml` as `valid_until:` on the entity (an RFC 3339 instant): the loader treats a declaration already past it as not declared, the export writes it back, and a window that differs between code and database is reported as drift on the Sync page. |
| Device certificate | Within the hour | The sweep stamps the certificate revoked, which is what the device gate reads. |

Re-adding a member whose earlier membership expired revives the row with the new window. For contractors, combine a membership expiry with a PAT expiry; account-level disable and revocation remain available for the immediate case.

## Context-aware access control

What a session can reach — marketplaces, plugins, skills, MCP servers, gateway routes — is decided per request from the person's **current context**: their role, the groups and projects they belong to, and the accounts they have linked. The narrowest band that names them decides, a deny beats an allow inside it, and an entity with rules is closed to everyone they do not name. Every rule is declared once in `services/access-control/rules.yaml` with a stated reason and enforced from the database; `/admin/access-control` shows who reaches what and why, and its Sync page reconciles code and console in either direction. The full model, worked examples and how-tos are in [Access Control](/documentation/access-control).

## Verification

Use the commands and UI checks above against the configured instance. Repository maintainers can run `just test-unit` and `just test-integration`; a passing fixture test does not establish that a deployed integration is configured.
