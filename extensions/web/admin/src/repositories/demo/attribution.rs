//! How AI usage is attributed to a tool invocation.
//!
//! `ai_requests` records no tool-call identifier and shares no `trace_id` with
//! `plugin_usage_events`, so no exact join exists. A request is attributed to
//! an invocation when it is the **same user** and its `created_at` falls in
//!
//! ```text
//! [ invoked_at , min( next invocation of the same kind in that session ,
//!                     last event of that session + ATTRIBUTION_PAD_MINUTES ) )
//! ```
//!
//! The pad exists because the model call that a tool invocation causes usually
//! completes after the session's final hook event, so an unpadded window drops
//! the tail of every session. Windows never overlap, so a request is counted
//! at most once per kind, and figures are labelled *attributed* on every page.

pub const ATTRIBUTION_PAD_MINUTES: i32 = 5;

pub const SKILL_WINDOW_PREDICATE: &str =
    "r.created_at >= bd.invoked_at AND r.created_at < bd.window_end";

pub const MCP_WINDOW_PREDICATE: &str =
    "r.created_at >= bd.invoked_at AND r.created_at < bd.window_end";
