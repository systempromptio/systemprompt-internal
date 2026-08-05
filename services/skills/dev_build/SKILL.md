# Build — Storefront Next Development

Build conventions for Salesforce Commerce Cloud work. The rules differ by context: **Storefront Next** (code-led, React Router based) and **classic B2C / SFRA** (cartridge-led). Identify which one the project uses before writing anything — the mounted project's own configuration and lockfile are the source of truth.

## When to Use

Use this skill while implementing an approved plan from `dev_plan`. It is the second stage of the Astound development workflow: plan → **build** → release → test.

## Identify the project shape first

- `react-router.config.*` / `app/routes` → **Storefront Next**: routes, loaders/actions, server-first data flow.
- `cartridges/*/cartridge/controllers` + ISML templates → **SFRA**: controller/pipeline extension model.
- PWA Kit (`pwa-kit.config`) → managed-runtime React storefront; follow its retail-react-app extension points.

Never mix idioms: no client-side data fetching where a route loader belongs, no new cartridge when an override in an existing site cartridge suffices.

## Storefront Next rules

- **Routes own data.** Fetch in route loaders/actions, not in components; components receive props. Mutations go through actions with progressive enhancement (forms work without JS).
- **Extend, don't fork.** Prefer the platform's documented extension points (route overrides, component slots, hooks) over copying platform source into the project.
- **Libraries are approved, not adopted.** Use what the project's `package.json` already ships. Proposing a new dependency is a `dev_plan` decision record, not a build-time choice.
- **Server-first.** Anything that can render on the server does; client components are the exception and say why.

## General build rules

- Match the surrounding code: naming, file layout, error handling, and comment density of the file you are editing.
- Small, single-purpose commits that map to the spec's step plan (format governed by `dev_release`).
- No dead flags or commented-out code left behind; delete, don't disable.
- Consult the `knowledge-bank` MCP server when a convention is unclear — prior project decisions outrank general best practice.
- Work is not complete until it passes verification under `dev_test` (Playwright).

## Astound rules

<!-- DROP-IN SECTION: Astound's build-category Cursor rules (React Router
     patterns, approved library list, SFCC specifics) land here verbatim once
     approved for sharing (owner: Roman). Keep the heading; replace this
     comment with the imported rules. -->

*Astound's organisation-specific build rules will appear here once imported.*
