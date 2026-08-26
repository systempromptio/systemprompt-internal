Subject: Everything's fixed and testable end to end — start here

Hey Oliver,

Big day of fixes — every blocker from your last emails is closed, and there is now one document that takes you through the whole flow from a fresh machine:

https://github.com/systempromptio/systemprompt-internal/blob/next/TESTING-INSTRUCTIONS.md

It has a human summary at the top, agent instructions at the bottom, and annotated screenshots of each surface here:

https://github.com/systempromptio/systemprompt-internal/tree/next/docs/testing

IMPORTANT — NEW BRANCH WORKFLOW (read this first)

Everything below lives on the "next" branch, and the workflow changed: you now need the next branch of BOTH repos, checked out as siblings.

- systemprompt-internal on branch next
- systemprompt-core on branch next, at ../systemprompt-core (right beside it)

Internal's next builds directly against the sibling core checkout (the crates-io patch is active on next), so the two branches move together — a fix landed in core this morning is in your build this afternoon, no release needed. Before you build or test anything: git pull both repos, then just build. Nothing runs against crates.io on next anymore; main stays the release branch and is not where you work. If a build ever complains about missing core crates, the sibling checkout is missing or on the wrong branch.

WHAT CHANGED SINCE WE LAST SPOKE

1. Your artifact allowlist bug — root-caused. The setup skill could install one dashboard's HTML under another's tool allowlist when it matched by name. It now matches by id only and verifies every installed allowlist, repairing mismatches itself. Delete the broken artifact, re-run the setup skill, done.

2. note_search "%" returning empty — a real server bug (wildcards were searched as literal percent signs). Fixed; Recent Activity populates.

3. Open Deals / Inbound Prospects showing nothing — the tools now return a typed table (structuredContent.items) instead of markdown the pages had to regex apart. Re-sync and accept the "stale" replacements when the setup skill offers them.

4. Login and roles — Odoo is now the role authority. Your Odoo groups map to platform roles at every sign-in (promotion AND demotion), and the device-link page shows which account it is linking, with a "use a different account" switch — the wrong-account-linked trap you hit is gone.

5. Bridge credential bugs — logout now purges sync state, the token cache is per-credential, and the "session user mismatch" 401s after switching users are fixed. Rebuild the bridge from next (both repos pulled, per the workflow above).

6. Two seeded test users — e2e-admin@systemprompt.local and e2e-sales@systemprompt.local (passwords in the instructions doc) with different roles. Switch between them and watch the skills, dashboards, and tools change. "just e2e" and "just e2e-live" run the entire journey automatically.

NEXT STEPS — UX LINKING BETWEEN THE SURFACES

The three surfaces (skills, inline tool results, dashboard artifacts) each work, but they don't point at each other yet. That is the next round, and it is a good one for you:

- An inline table result should link to its dashboard ("open Pipeline board"), and the dashboard back to the record in Odoo.
- Dashboards should cross-link — a lead row opens that lead's Recent Activity notes.
- The setup skill's receipt should link to what it installed.
- A styling pass on the inline tables (core's ui_renderer table.css against the shared design tokens — section 7b of the instructions explains who owns what).

Section 6c of the instructions is the template for shipping any of these end to end (tool → artifact → skill, with tests). Get the flow green on your machine first, then pick one of the linking items and go.

Ed
