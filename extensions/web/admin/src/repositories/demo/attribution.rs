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
//!
//! The predicate itself is spelled out in each query rather than shared as a
//! constant, because `sqlx::query_as!` needs a literal and cannot interpolate
//! one. What *is* shared lives in the `skill_invocation_events` view (see
//! `schema/05_plugin_usage.sql`), which is where the definition of an
//! invocation belongs: the database can hold it once for every reader.

pub const ATTRIBUTION_PAD_MINUTES: i32 = 5;
