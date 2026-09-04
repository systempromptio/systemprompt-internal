//! The Systemprompt Internal [`ArtifactTheme`].
//!
//! Core's `ui_renderer` ships deliberately unbranded neutral-slate tokens so an
//! untethered deployment still looks composed, and expects the deployment to
//! re-declare whichever `--mcpui-*` properties it cares about. Until this file
//! existed we registered none, so every artifact rendered into Cowork — the CRM
//! lead table, every chart, every dashboard — came back cool-blue and
//! square-cornered while the rest of the product is warm orange with a notched
//! top-right corner.
//!
//! This crate is the host because all three MCP servers
//! (`odoo`, `knowledge-bank`, `systemprompt`) already depend on it, so one
//! registration reaches all three binaries. `extensions/brand` would have been
//! the intuitive home and is the wrong one: it is a leaf crate the standalone
//! `bridge/` workspace also builds, and a `systemprompt` dependency there would
//! drag the whole core into the bridge.
//!
//! **This registration reaches a binary only because that binary already calls
//! something else in this crate.** `inventory` puts its registration in a
//! static, and a linker drops an rlib nothing references — object file, static
//! and all. Every MCP server here calls `record_mcp_access` (audit logging is
//! not optional), so the crate is always linked and the theme always arrives;
//! verified by `strings <binary> | grep '0.67 0.18 50'` returning hits for all
//! four servers. A server that stopped calling into this crate would silently
//! lose its branding with nothing failing to compile — so if the audit helpers
//! ever move out, an explicit anchor has to move in.
//!
//! Values are literal copies of `storage/files/css/core/tokens-primitives.css`
//! and `core/tokens.css`, not `var(--sp-*)` references — the artifact renders
//! inside a sandboxed `srcdoc` iframe where the site's stylesheets are not in
//! scope, so a `var()` pointing at them resolves to nothing.
//!
//! The theme has two slots and this file now uses both. `tokens` re-declares
//! `--mcpui-*` values and lands in `:root` before every renderer stylesheet;
//! `extra_css` is appended *after* them, so it can override a rule and not just
//! a value. The split is the useful boundary: anything expressible as a token
//! belongs in `TOKENS`, and `EXTRA_CSS` is reserved for the few facts about
//! *where* this deployment's artifacts render — inside Cowork's own tool-call
//! card, which already supplies a header and a frame. A restyle that would be
//! right for every deployment belongs in core's stylesheets instead, not here.

use systemprompt::mcp::register_artifact_theme;
use systemprompt::mcp::services::ui_renderer::ArtifactTheme;

// Why: declarations only — core wraps this in `:root { … }` (`html.rs:52`),
// after its own tokens and before every renderer stylesheet, so what is named
// here wins and everything else is inherited.
const TOKENS: &str = r#"
  /* ── surfaces ── warm neutrals (hue ~50-70), not core's cool 255. */
  --mcpui-bg:             light-dark(oklch(1.00 0 0),      oklch(0.20 0.01 50));
  --mcpui-surface:        light-dark(oklch(0.985 0.004 70), oklch(0.25 0.01 50));
  --mcpui-surface-raised: light-dark(oklch(1.00 0 0),      oklch(0.29 0.01 50));
  --mcpui-surface-sunken: light-dark(oklch(0.97 0.005 70), oklch(0.27 0.01 50));
  --mcpui-border:         light-dark(oklch(0.92 0.008 65), oklch(0.34 0.01 55));
  --mcpui-border-strong:  light-dark(oklch(0.86 0.010 60), oklch(0.41 0.01 60));

  /* ── ink ── */
  --mcpui-ink:       light-dark(oklch(0.20 0.01 50), oklch(0.98 0.004 70));
  --mcpui-ink-dim:   light-dark(oklch(0.41 0.01 60), oklch(0.86 0.010 60));
  --mcpui-ink-faint: light-dark(oklch(0.53 0.01 65), oklch(0.70 0.01 70));

  /* ── accent ── the brand orange (--sp-color-primary). */
  --mcpui-accent:      light-dark(oklch(0.67 0.18 50), oklch(0.72 0.17 52));
  --mcpui-accent-ink:  light-dark(oklch(1.00 0 0),     oklch(0.13 0.01 45));
  --mcpui-accent-wash: color-mix(in oklab, var(--mcpui-accent) 12%, transparent);
  --mcpui-accent-rim:  color-mix(in oklab, var(--mcpui-accent) 42%, transparent);

  /* ── status ── */
  --mcpui-success: light-dark(oklch(0.72 0.19 155), oklch(0.78 0.16 165));
  --mcpui-warning: light-dark(oklch(0.83 0.16 85),  oklch(0.76 0.16 75));
  --mcpui-danger:  light-dark(oklch(0.63 0.21 25),  oklch(0.70 0.17 20));
  --mcpui-info:    light-dark(oklch(0.62 0.18 260), oklch(0.70 0.14 250));

  /* ── chart series ──
   * Core's ordering keeps adjacent pairs separable at small sizes and under the
   * common colour-vision deficiencies; that rationale is sound, so this re-hues
   * in place rather than reordering. Series 1 is the brand orange, and 2 is
   * pushed to blue rather than the neighbouring amber so the two most common
   * (single- and two-series) charts stay legible under deuteranopia. */
  --mcpui-series-1: light-dark(oklch(0.67 0.18 50),  oklch(0.72 0.17 52));
  --mcpui-series-2: light-dark(oklch(0.62 0.18 260), oklch(0.70 0.14 250));
  --mcpui-series-3: light-dark(oklch(0.68 0.18 300), oklch(0.80 0.12 300));
  --mcpui-series-4: light-dark(oklch(0.72 0.19 155), oklch(0.80 0.18 155));
  --mcpui-series-5: light-dark(oklch(0.79 0.13 200), oklch(0.87 0.10 200));
  --mcpui-series-6: light-dark(oklch(0.70 0.17 350), oklch(0.80 0.12 350));

  /* ── radii ── the branded asymmetric corner.
   * Core documents these as four-value TL/TR/BR/BL slots precisely so a brand
   * with a notch can drop in; the top-right is a quarter of the others. */
  --mcpui-radius-card:  0.75rem 0.25rem 0.75rem 0.75rem;
  --mcpui-radius-inner: 0.5rem 0.1875rem 0.5rem 0.5rem;
  --mcpui-radius-sm:    0.3125rem 0.125rem 0.3125rem 0.3125rem;
  /* pill stays symmetric: a notched pill reads as a rendering fault. */

  /* ── elevation ── warm-tinted, with an orange cast on the raised state. */
  --mcpui-shadow-card:   light-dark(0 1px 3px oklch(0.20 0.01 50 / 0.06),
                                    0 1px 3px oklch(0 0 0 / 0.25));
  --mcpui-shadow-raised: light-dark(0 12px 28px oklch(0.20 0.01 50 / 0.08),
                                    0 4px 12px oklch(0.67 0.18 50 / 0.08));
  --mcpui-shadow-inset:  inset 0 1px 0 light-dark(oklch(1 0 0 / 0.7), oklch(1 0 0 / 0.045));

  /* ── type ──
   * The webfont names lead so a host that already has them installed picks them
   * up, but they cannot be fetched: the artifact is a `srcdoc` iframe on an
   * opaque origin under `default-src 'self'`, so any @font-face URL resolves to
   * nothing. The system stack behind them is what actually renders. */
  --mcpui-font-heading: "Inter", system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  --mcpui-font-body:    "OpenSans", system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  --mcpui-font-mono:    ui-monospace, "Cascadia Code", "Fira Code", Menlo, monospace;

  /* Fluid scale from core/tokens-primitives.css. `vw` is safe here: the frame
   * negotiates its height, so type that responds to width cannot feed back. */
  --mcpui-text-xs:  clamp(0.7rem, 0.66rem + 0.2vw, 0.75rem);
  --mcpui-text-sm:  clamp(0.8rem, 0.75rem + 0.25vw, 0.875rem);
  --mcpui-text-md:  clamp(0.875rem, 0.82rem + 0.28vw, 0.9375rem);
  --mcpui-text-lg:  clamp(1rem, 0.93rem + 0.35vw, 1.125rem);
  --mcpui-text-xl:  clamp(1.125rem, 1rem + 0.5vw, 1.25rem);
  --mcpui-text-2xl: clamp(1.4rem, 1.2rem + 1vw, 1.75rem);

  --mcpui-tracking-tight: -0.02em;

  /* ── spacing ── compressed for a card inside a card.
   * Core's scale (0.25 → 2rem) is sized for an artifact that owns a page. Ours
   * never does: it renders in a sandboxed iframe that Cowork has already
   * wrapped in its own padded, bordered tool-call card, inside a narrow chat
   * column. At core's scale the padding is paid twice and a six-row table costs
   * a screenful. Every renderer stylesheet spends only these six properties, so
   * re-declaring them here tightens tables, text, cards, lists and dashboards
   * in one place rather than rule by rule. */
  --mcpui-space-1: 0.1875rem;
  --mcpui-space-2: 0.375rem;
  --mcpui-space-3: 0.5rem;
  --mcpui-space-4: 0.75rem;
  --mcpui-space-5: 1rem;
  --mcpui-space-6: 1.25rem;

  --mcpui-leading-normal: 1.45;

  --mcpui-ease: cubic-bezier(0.4, 0, 0.2, 1);
"#;

// Why: what a custom property cannot express. `extra_css` is appended after
// every renderer stylesheet (`html.rs`), so these override rules rather than
// values — which is exactly why the scope is kept to the two things that are
// true of *this* deployment's placement and would be wrong to push into core:
// the artifact is not the page, and it is not the card.
const EXTRA_CSS: &str = r#"
/* Cowork's tool-call card already carries a header naming the server and the
 * tool — "odoo · activity_list" — directly above this iframe. The artifact's
 * own <h1> restated it one line later ("Odoo Activities") under an accent rule,
 * which cost roughly 70px per artifact to say the same thing twice. The title
 * stays on the artifact struct: it still fills the document <title> and any
 * surface that renders an artifact on its own. Only the duplicate is hidden. */
.mcp-app-title {
  display: none;
}

/* Cowork draws the border, the radius and the elevation around the frame. A
 * second raised card inside the first reads as a rendering fault, so the
 * content blocks keep a hairline and drop the shadow. */
.table-wrapper,
.text-content,
.card {
  box-shadow: none;
}

/* The description is the first thing left once the title is gone, so it stops
 * being offset for a heading that is no longer above it. */
.mcp-app-title + .mcp-app-description {
  margin-top: 0;
}
"#;

register_artifact_theme!(
    || ArtifactTheme::new(TOKENS).with_extra_css(EXTRA_CSS),
    name = "systemprompt-internal"
);
