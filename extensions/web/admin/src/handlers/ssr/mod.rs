//! Server-rendered admin pages.
//!
//! Each module owns one page: it builds a typed template context and renders a
//! `.hbs` template from `storage/files/admin/templates/` at request time.

use crate::error::{AdminHtmlError, AdminHtmlResult};
use crate::templates::AdminTemplateEngine;
use axum::Extension;
use axum::response::{Html, IntoResponse, Response};


mod context;
pub(crate) mod entity_urls;
pub(crate) mod format;
pub(crate) mod list_view;
pub(crate) mod ssr_analytics_requests;
mod ssr_bridge_device_link;
mod ssr_bridge_setup;
mod ssr_chain;
mod ssr_context_detail;
mod ssr_conversations_raw;
mod ssr_demo_help;
mod ssr_enterprises;
mod ssr_governance_audit_detail;
pub(crate) mod ssr_helpers;
mod ssr_management;
mod ssr_perf_trace_detail;
mod ssr_perf_traces;
mod ssr_profile;
mod ssr_report_customer;
mod ssr_report_internal;
mod ssr_search_resolve;
mod ssr_session_detail;
mod ssr_settings;
mod ssr_setup;
mod ssr_skills_contexts;
mod ssr_users;
mod ssr_users_sessions;
pub(crate) mod types;

pub(crate) use ssr_analytics_requests::analytics_requests_page;
pub(crate) use ssr_bridge_device_link::{device_link_approve, device_link_deny, device_link_page};
pub(crate) use ssr_bridge_setup::bridge_setup_page;
pub(crate) use ssr_chain::chain_envelope;
pub(crate) use ssr_context_detail::context_detail_page;
pub(crate) use ssr_conversations_raw::conversations_raw;
pub(crate) use ssr_enterprises::{enterprise_detail_page, enterprises_page};
pub(crate) use ssr_governance_audit_detail::governance_audit_detail_page;
pub(crate) use ssr_helpers::{branding_context, render_typed_page};
pub(crate) use ssr_management::{management_department_detail_page, management_departments_page};
pub(crate) use ssr_perf_trace_detail::perf_trace_detail_page;
pub(crate) use ssr_perf_traces::perf_traces_page;
pub(crate) use ssr_profile::profile_page;
pub(crate) use ssr_report_customer::report_customer_page;
pub(crate) use ssr_report_internal::report_internal_page;
pub(crate) use ssr_search_resolve::search_resolve;
pub(crate) use ssr_session_detail::session_detail_page;
pub(crate) use ssr_settings::settings_page;
pub(crate) use ssr_setup::setup_page;
pub(crate) use ssr_skills_contexts::skills_contexts_page;
pub(crate) use ssr_users::{user_detail_page, users_page};
pub(crate) use ssr_users_sessions::users_sessions_page;

pub(crate) async fn login_page(
    Extension(engine): Extension<AdminTemplateEngine>,
) -> AdminHtmlResult<Response> {
    render_unauthenticated(&engine, "login")
}

/// The pages reachable before sign-in, which therefore have no user or
/// marketplace context to inject and cannot go through `render_page`.
fn render_unauthenticated(
    engine: &AdminTemplateEngine,
    template: &str,
) -> AdminHtmlResult<Response> {
    let html = engine
        .render(template, &branding_context(engine))
        .map_err(|e| AdminHtmlError::internal(format!("{template} page render failed: {e:?}")))?;
    Ok(Html(html).into_response())
}
