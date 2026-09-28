---
title: "Access Control: Who Reaches What, and Why"
description: "How entitlement works on this instance: the subject bands and their precedence, the one declarative file every rule lives in, the database that enforces it, and the sync loop that keeps code and console in step."
author: "systemprompt.io"
slug: "access-control"
keywords: "access control, entitlement, rules.yaml, roles, groups, projects, marketplace, plugin, skill, mcp server, gateway route, precedence, deny overrides, sync, drift, export"
kind: "guide"
public: true
tags: ["documentation", "admin", "access"]
published_at: "2026-09-28"
updated_at: "2026-09-28"
after_reading_this:
  - "Say, for any entity, who reaches it and which band decided"
  - "Predict the resolver's answer for a person from their role, group and project"
  - "Grant a group a tool, make an entity admin-only, or retire a rule"
  - "Tell what is enforced (the database) from what is declared (rules.yaml), and reconcile the two in either direction"
related_docs:
  - title: "Authentication"
    url: "/documentation/authentication"
  - title: "Connect Odoo"
    url: "/documentation/odoo"
  - title: "Code ↔ Instance: sources, planes and sync"
    url: "/documentation/services-sync"
---

# Access Control: Who Reaches What, and Why

**TL;DR:** Every workspace, plugin, skill, MCP server and model route a signed-in person can reach is decided by rules on that entity. The rules are **declared once**, in `services/access-control/rules.yaml`, each with a stated reason, and **enforced from the database**. A decision walks the bands from narrowest to widest — *person → project → group → role → organization* — and the first band that names the person decides, with a deny beating an allow inside it. Code and database are compared on every boot and **never silently merged**.


## What access control decides — and what it does not

Access control answers one question: *may this person reach this entity?* The entities are:

| Kind | Examples on this instance | What "reach" means |
|---|---|---|
| `marketplace` | `enterprise-demo` | The workspace appears in Claude Code / Cowork and its plugins can be installed |
| `plugin` | `systemprompt-business`, `systemprompt-admin` | The plugin is offered |
| `skill` | any skill a plugin ships | The skill is offered and may run |
| `mcp_server` | `odoo`, `knowledge-bank`, `systemprompt` | The server is listed and its tools may be called |
| `gateway_route` | `claude-star-4203d1`, … | Requests may be routed to that model |
| `slack_channel` | every channel of the connected workspace | Messages from the channel reach the platform |

It does **not** decide whether a person can sign in (that is [authentication](/documentation/authentication)), whether a tool call may proceed once the person is entitled (the governance chain in `services/governance/config.yaml`), or what may be said (the safety scanners in `services/gateway/policies.yaml`). Those run after entitlement, on every request. Nor does it decide what a person may see *inside* Odoo: every Odoo call runs as that person, so Odoo's own record rules apply ([Connect Odoo](/documentation/odoo)).

## The model

### Subjects and bands

A rule names a **subject** at one **band**:

| Band | Precedence | Subject is… | Where a person gets it |
|---|---|---|---|
| person | 0 | one account | an override set on the user's Access tab |
| project | 140 | a project id | assigned on the Projects page or mapped from a directory group |
| group | 150 | a group id | mapped from the directory groups asserted at sign-in |
| role | 200 | `user`, `admin` | granted at sign-in (Odoo groups map to roles through `odoo-roles.yaml`) or on the Roles page |
| organization | 300 | an organization slug | the organization's plan in `services/access-control/plans.yaml` |

Lower precedence is **narrower**. The ladder is walked from the top.

### How a decision is made

1. **The narrowest band in which the person matches any rule decides.** A band in which nothing names them is skipped entirely — a group rule for a group they are not in has no effect on them, good or bad.
2. **Inside that band a deny beats an allow.**
3. **An entity with any rule is closed to everyone its rules do not name**, unless it is marked `default: open`. The rules are the whole story for that entity.
4. **An entity with no rule at all inherits from its parent**: a skill from its plugin, a plugin from its marketplace. This is why skills need no rule of their own — they follow the plugin that ships them.

Two consequences worth holding onto:

- A **role grant does not "reopen" an entity to everyone** — only to the roles it names. `knowledge-bank` is allowed to `admin`; a plain `user` is refused by the closed default.
- A **wider band cannot rescue a narrower deny**. A person-band deny on one account beats every group and role allow that person holds.

### Worked examples from this instance

`plugin/systemprompt-admin` is closed and allows role `admin`. `marketplace/enterprise-demo` is open and allows role `user`.

| Person | Walk | Result |
|---|---|---|
| role `user` | the plugin's own rule names only `admin`; the plugin declares a rule, so the marketplace grant does not cascade into it | **refused** — the entity is closed |
| roles `user`, `admin` | role band matches the plugin's allow | **allowed** (decided by role) |
| role `user`, one of its skills | the skill has no rule, so it inherits the plugin's closed answer | **refused** |

## The file: `services/access-control/rules.yaml`

Every rule on the instance is declared in this one file, entity by entity. Marketplaces are entities like any other — there is no second place where a workspace's audience is written.

```yaml
entities:
  - entity: mcp_server/knowledge-bank   # <kind>/<id>
    default: closed                      # open | closed — what an unmatched person gets
    why: >-                              # REQUIRED; becomes the justification on every rule row
      The curated reference documents and the inbox pipeline behind them;
      administrators only.
    allow:                               # band → subjects
      role: [admin]
    deny: {}                             # same shape

  - entity: gateway_route/*              # glob: route ids are generated, never written
    default: open
    why: Every model route is reachable by every signed-in role.
    allow:
      role: [user, admin]
```

Rules of the file, all checked by `scripts/validate-services.sh` in CI:

- `why` is required and non-empty. A rule with no reason is a rule nobody can review.
- Every `entity` must exist: plugins, skills, agents and MCP servers in `services/`, marketplaces under `services/marketplaces/`.
- `gateway_route`, `hook` and `slack_channel` take only the glob `*`, expanded against the live catalog. A hand-written route id names a route that cannot exist; a channel id is Slack's, not ours.
- Band keys are `role`, `group`, `project`, `connector`. Every `group:`/`project:` value must be declared in `services/web/config/groups.yaml`.
- A subject may not be both allowed and denied on the same band of one entity.
- Every plugin is declared exactly once, with a role allow of `[user]` (`default: open`) or `[admin]` (`default: closed`). A skill entity may only deny.
- `valid_until` is optional, an RFC 3339 instant (`2026-12-31T00:00:00Z`), and applies to every rule of the entity. A declaration already past it is treated as not declared; the hourly `access_expiry` sweep deletes the rows once the instant passes.
- **No `services/marketplaces/*/config.yaml` may carry an `access:` block.** The gate refuses it, so the second truth cannot return.

Per-person overrides (the person band) have no place in the file. They belong to one account, are set on that person's Access tab, and are never synced or exported. Organization grants stay in `plans.yaml`, where a plan's grants are projected onto each organization on it.

## Database and console

The database — `access_control_rules` and `access_control_entities` — is what the resolver reads. It is also what the console edits: a group's Access tab sets that group's band on any entity (allow or deny asks for a **reason**), a person's Access tab sets a person-band override. Every console write is stamped `source = dashboard`, so the sync page can tell a console decision from one the file wrote.

## The sync loop

Code and database are two copies of the same intent. Either may be edited; neither is silently overwritten.

**On boot** the server reads `rules.yaml` and, if the database holds **no** band rules at all (a fresh install), seeds it from the file in one transaction. Otherwise it computes the differences, logs them, and **writes nothing**. A deploy that adds a rule to the file therefore does not land that rule until an administrator chooses to.

**The sync page** offers three directions:

| Action | Writes | Leaves alone |
|---|---|---|
| **Insert only** | Rules and entity defaults the file declares that the database lacks | Everything that already exists, even if it differs |
| **Overwrite from code** | Adds what code added, corrects what code changed, and **deletes** what code removed | Rows the console wrote on entities the file does not mention; every person-band override |
| **Export to code** | Nothing in the database. Renders it as a `rules.yaml` to copy or download | — |

The asymmetry is the point. Code is the source for everything code wrote; the console is the source for what it wrote, and a console row reaches the file only through **Export** — the instance never writes into the repository itself. How sources, planes and hashes fit together: [Code ↔ Instance](/documentation/services-sync).

## External kits

A marketplace can arrive as a signed **services bundle** published by another repository — a *kit* — rather than from this repository's `services/`. **Kits carry no access.** Who reaches a kit's marketplace is declared here, like every local one, with one extra key naming the source it arrives from:

```yaml
  - entity: marketplace/partner-tools
    owner: bundle:partner
    default: closed
    why: Partner workspace, published by its own kit repository.
    allow:
      role: [admin]
```

`owner: bundle:<name>` lets the entity be declared before the bundle is pinned: until that source is active it is listed as **awaiting its bundle** rather than failing boot, and the CI gate accepts the id without a local `services/marketplaces/<id>`. A kit release therefore can never widen who sees it — ownership of *content* is the kit's; ownership of *access* is this repository's.

## How-tos

**Grant a group a tool.** Declare the group in `services/web/config/groups.yaml`, add it to the entity's `allow.group` list in `rules.yaml`, and sync.

**Make an entity admin-only.** Declare it with `default: closed` and `allow: { role: [admin] }`. Declaring any rule opts the entity out of the marketplace cascade, which is what makes the list restrictive rather than additive.

**Retire a rule — or a whole entity — cleanly.** Remove it from `rules.yaml`, deploy, Sync → Overwrite. Insert only would leave it in place.

## Glossary

- **Entity** — the thing reached: marketplace, plugin, skill, MCP server, gateway route, Slack channel.
- **Subject** — who a rule names: a person, project, group, role or organization.
- **Band** — the kind of subject, with a precedence number; lower is narrower.
- **Default open / closed** — what an entity gives to a person no rule names. Closed unless declared otherwise.
- **Cascade** — a ruleless entity's inheritance from plugin then marketplace.
- **Declared** — what `rules.yaml` says. **Enforced** — what the database holds.
- **Drift** — any difference between the two.
