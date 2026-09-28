---
title: "Code ↔ Instance: Sources, Planes and Sync"
description: "One sync system for every declared service, local and external: where declarations come from (sources), what they project into (planes), which hash proves what, the three directions, and how a kit published from another repository reaches the instance without a redeploy."
author: "systemprompt.io"
slug: "services-sync"
keywords: "sync, services bundle, kit, marketplace, digest, content hash, composed hash, declared hash, provenance, refresh, pin, rules.yaml, owner, access control, drift"
kind: "guide"
public: true
tags: ["documentation", "admin", "operations"]
published_at: "2026-09-28"
updated_at: "2026-09-28"
after_reading_this:
  - "Say, for every kind of configuration, where it comes from and whether the database agrees with the code"
  - "Tell the four hashes apart and know which one a pin, an import and an apply each change"
  - "Ship a kit from its own repository to the instance: publish, pin, import, verify"
  - "Reconcile a plane in either direction and read back who applied what, when, from which declaration"
related_docs:
  - title: "Access Control: Who Reaches What, and Why"
    url: "/documentation/access-control"
  - title: "Standing Up the Gateway"
    url: "/documentation/use-case-admin"
---

# Code ↔ Instance: Sources, Planes and Sync

**TL;DR:** Everything this instance serves is *declared* somewhere in code and *enforced* from what the process loaded and what the database holds. A **source** is where declarations come from — `base` is this repository's `services/` tree, `bundle:<name>` is each external kit the profile pins by digest — and every source has a content hash. A **plane** is what a source's declarations project into the database — access control, groups and projects, gateway policies, gateway routes, the governance chain — and every plane records the declared hash it last applied, when, by whom and in which mode. Nothing writes on its own after the first seed: the console shows the difference and offers a direction.


## The model

| word | meaning |
|---|---|
| **source** | a tree of declarations with a content hash: `base` (this repository), plus any kit the profile pins |
| **plane** | a projection of declarations into database tables, with drift and three directions |
| **ownership** | decided by id at composition: a bundle owns its marketplaces, plugins and skills; the base owns everything else, every entitlement included |
| **hash control** | every source reports what it declares; every plane records what it applied |

| plane | declared in | projected into | the other writer |
|---|---|---|---|
| `access_control` | `access-control/rules.yaml` | `access_control_rules`, `access_control_entities` | the access-control console (rows stamped `dashboard`) |
| `groups` | `web/config/groups.yaml` | `groups`, `projects` and their directory mappings | the Groups and Projects pages. **Never deletes a group or project** — those own members, usage and rules |
| `gateway_policies` | `gateway/policies.yaml` | `ai_gateway_policies` | the gateway policies page — live within a minute, because core reads the table per request |
| `gateway_routes` | `ai/gateway.yaml` `routes:` | `gateway_routes` | the gateway page. **Core boots the dispatcher from the file and never reads the table**; a console change is dispatched at the next restart |
| `governance` | `governance/config.yaml` | `governance_chain`, `governance_chain_settings` | none — core builds the chain from the file at boot; a staged change is enforced after export, commit and restart |

### One boot contract

Every plane meets the same rule at boot, in the order groups → access control → gateway policies (the rules name group and project ids, so those must exist first). If the plane's projection is **empty**, boot seeds it from code, recorded in `sync_state` with actor `boot` and mode `seed`. Otherwise boot **compares and writes nothing**: the drift is logged and the console shows it. A restart therefore never undoes a console edit, on any plane. A declaration that cannot be read — the file does not parse, or the services tree it validates against does not compose — fails the boot rather than starting a server on half a declaration.

**People are never declared in code.** Group and project membership, manually granted roles and per-person access overrides live only in the database and the directory; no plane declares them and no export carries them.

## The four hashes

| hash | comes from | changes when | proves |
|---|---|---|---|
| bundle **digest** `sha256:…` | the registry, per pushed artifact | a kit publishes a release | *which upload* — this is what the profile pins |
| bundle **content hash** | `bundle.json`: SHA-256 over sorted `path\0sha256` | the kit's files change | *which tree* — the same tree pushed twice has one content hash |
| **composed hash** | SHA-256 over ordered `name\0content_hash` of every active source | any source changes | *which composition* the process is serving |
| plane **declared hash** | SHA-256 over the canonical declaration | the file's meaning changes (not its whitespace) | what the database was last made equal to; recorded per apply in `sync_state` |

A marketplace's own **version** is its content hash — sha256 over its config, every plugin it includes and every skill they ship — recorded in `marketplace_versions` at every boot, so a skill invocation can be credited to the exact tree it ran from.

## The three directions

| action | writes | leaves alone |
|---|---|---|
| **Insert only** | rows the declaration carries and the database lacks | everything that exists, even if it differs |
| **Overwrite from code** | adds what code added, corrects what code changed, **deletes** what code removed | rows the console or a bundle wrote on entities the declaration does not mention; every per-person override |
| **Export** | nothing — renders the database as the declaration file to commit; the only way a console row reaches code | — |

Every apply runs in one transaction that re-reads the tables first, writes an activity row (who, plane, mode, counts), and updates `sync_state` with the declared hash it applied.

## Kits: content from another repository

A **kit** is a repository in Anthropic marketplace format — `.claude-plugin/marketplace.json`, `plugins/<id>/` — plus two small systemprompt sidecars. Its CI publishes a signed **services bundle** on every release; the instance composes it with `services/` at boot. A new kit is seeded from this repository with `just kit-export`, and the template lives in `deploy/kit/`.

**Kits own content; this repository owns access.** A kit ships no `access:` block. Who reaches a kit's marketplace is declared in `services/access-control/rules.yaml` here with `owner: bundle:<name>`, which lets the entity be declared before the digest is pinned.

### End to end

1. **Kit CI** validates every push and pull request (sanitize, import, compose under the base) and, on a merge that bumps `metadata.version`, packs and signs the bundle, publishes it with the kit's channel tag moved to the same digest, and asks the instance to import.
2. **Pin**, once: `just services-pin <kit> stable` makes the profile follow the channel (image, public key and pull mode from `deploy/kit/known-kits.json`); `just services-pin <kit> sha256:<digest>` pins one upload instead. The instance never rewrites its own profile.
3. **Import**: the kit CI's call, or **Import sources** on the sync page. The reply says `changed`, `reconciled` and `restart_recommended`; the last is true only for a kit that ships governance hooks, which the process reads once at boot.
4. **Green**: the kit's source row is *in step* with the new active digest and the marketplace is listed. The process never restarted.

### Rollback

`just services-pin <kit> <previous digest>` and Import. The cache keeps the last two content-addressed trees per source, so a rollback is a re-point, not a download.

### What fails boot

An id — marketplace, plugin or skill — present both in `services/` and in a bundle: composition names both sources and refuses. A bundle whose signature does not verify against the pinned public key, or whose digest does not match the pin. A second source carrying `config/`, `gateway/` or `access-control/`: only the base source may. A declaration a plane cannot read. Everything else — a fetch that fails, a kit declaring access, an entity awaiting its bundle — is reported, not fatal.

## Where things are

| what | where |
|---|---|
| the state | table `sync_state`, one row per plane; `service_sources` / `service_owned_ids` for composition |
| the kit template and registry | `deploy/kit/` — runbook, sidecars, `known-kits.json` |
| the exporter | `just kit-export <marketplace> <dir>` — writes a kit and proves it re-imports identically |
| the pin | `just services-pin <kit> <digest \| channel> [profile]` |
| how to run a kit elsewhere | `docs/kits-on-another-instance.md` in the repository |
