//! Display formatting shared by the admin entity pages.
//!
//! The pure value formatters live in `systemprompt_web_shared::format` (so
//! the test workspace can exercise them); this module re-exports them for
//! the ssr pages and keeps the admin-specific helpers.
//!
//! Ids, costs, token counts and durations are rendered in a handful of places
//! across the entity list and detail pages; the rules live here so a cost reads
//! the same on the sessions list as it does on the session it links to.

pub(crate) use systemprompt_web_shared::format::{format_cost, format_duration_ms, short_num};

pub(crate) fn format_token_total(total: i64) -> String {
    if total <= 0 {
        return "—".to_owned();
    }
    short_num(total)
}
