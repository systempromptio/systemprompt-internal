Subject: Everything's fixed and testable end to end — start here

Hey Oliver,

Big day of fixes — every blocker from your last emails is closed, and there's
now one document that takes you through the whole flow from a fresh machine:

**[TESTING-INSTRUCTIONS.md](https://github.com/systempromptio/systemprompt-internal/blob/next/TESTING-INSTRUCTIONS.md)**
(repo root, `next` branch — human summary at the top, agent instructions at
the bottom, screenshots of each surface in
[docs/testing/](https://github.com/systempromptio/systemprompt-internal/tree/next/docs/testing)).

What actually changed since we last spoke:

1. **Your artifact allowlist bug** — root-caused: the setup skill could
   install one dashboard's HTML under another's tool allowlist when it
   matched by name. It now matches by id only and verifies every installed
   allowlist, repairing mismatches itself. Delete the broken artifact,
   re-run the setup skill, done.
2. **note_search "%" returning empty** — a real server bug (wildcards were
   searched as literal percent signs). Fixed; Recent Activity populates.
3. **Open Deals / Inbound Prospects showing nothing** — the tools now return
   a typed table (`structuredContent.items`) instead of markdown the pages
   had to regex apart. Re-sync + accept the "stale" replacements.
4. **Login/roles** — Odoo is now the role authority: your Odoo groups map to
   platform roles at every sign-in (promotion AND demotion), and the
   device-link page shows which account it's linking with a "use a
   different account" switch — the wrong-account-linked trap is gone.
5. **Bridge credential bugs** — logout now purges sync state, the token
   cache is per-credential, and the "session user mismatch" 401s after
   switching users are fixed. Rebuild the bridge from `next` (both repos'
   `next` branches now build together).
6. **Two seeded test users** (`e2e-admin@` / `e2e-sales@systemprompt.local`)
   with different roles — switch between them and watch the skills,
   dashboards, and tools change. Full walkthrough in the doc, including
   `just e2e` / `just e2e-live`, which run the entire journey automatically.

**Next steps — UX linking between the surfaces.** The three surfaces (skills,
inline tool results, dashboard artifacts) each work, but they don't point at
each other yet. That's the next round, and it's a great one for you:

- An inline table result should link to its dashboard ("open Pipeline board")
  and the dashboard back to the record in Odoo.
- Dashboards should cross-link (a lead row → its Recent Activity notes).
- The setup skill's receipt should link to what it installed.
- Styling pass on the inline tables (core's `ui_renderer/templates/assets/css/table.css`
  against the `tokens.css` design tokens — the doc's section 7b explains the
  ownership).

Section 6c of the instructions is the template for shipping any of these end
to end (tool → artifact → skill, with the tests). Have a look, get the flow
green on your machine first, and pick one of the linking items to start.

Ed
