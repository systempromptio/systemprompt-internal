//! Server-rendered admin pages.
//!
//! Each module owns one page: it builds a typed template context and renders a
//! `.hbs` template from `storage/files/admin/templates/` at request time.

use crate::error::AdminHtmlResult;
use crate::templates::AdminTemplateEngine;
use axum::Extension;
use axum::extract::Query;
use axum::response::{Html, IntoResponse, Response};
use systemprompt::identifiers::ClientId;


mod context;
pub(crate) mod entity_urls;
pub(crate) mod format;
pub(crate) mod list_view;
pub(crate) mod ssr_analytics_requests;
mod ssr_approvals;
mod ssr_approvals_ingest;
mod ssr_bridge_device_link;
mod ssr_bridge_device_link_view;
mod ssr_bridge_setup;
mod ssr_chain;
mod ssr_context_detail;
mod ssr_conversations_raw;
mod ssr_demo;
mod ssr_demo_help;
mod ssr_enterprises;
mod ssr_governance_audit_detail;
mod ssr_governance_dashboard;
mod ssr_governance_decisions;
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
mod ssr_skill_usage;
mod ssr_skills_contexts;
mod ssr_users;
mod ssr_users_sessions;
pub(crate) mod types;

pub(crate) use ssr_analytics_requests::analytics_requests_page;
pub(crate) use ssr_approvals::{approval_approve, approval_deny, approvals_page};
pub(crate) use ssr_bridge_device_link::{
    device_link_approve, device_link_deny, device_link_page, device_link_switch,
};
pub(crate) use ssr_bridge_setup::bridge_setup_page;
pub(crate) use ssr_chain::chain_envelope;
pub(crate) use ssr_context_detail::context_detail_page;
pub(crate) use ssr_conversations_raw::conversations_raw;
pub(crate) use ssr_demo::{demo_logbook_page, demo_me_page, demo_skills_page, demo_tools_page};
pub(crate) use ssr_enterprises::{enterprise_detail_page, enterprises_page};
pub(crate) use ssr_governance_audit_detail::governance_audit_detail_page;
pub(crate) use ssr_governance_dashboard::governance_dashboard_page;
pub(crate) use ssr_governance_decisions::governance_decisions_page;
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
pub(crate) use ssr_skill_usage::skill_usage_page;
pub(crate) use ssr_skills_contexts::skills_contexts_page;
pub(crate) use ssr_users::{user_detail_page, users_page};
pub(crate) use ssr_users_sessions::users_sessions_page;

#[derive(serde::Deserialize)]
pub(crate) struct LoginParams {
    redirect: Option<String>,
    client_id: Option<ClientId>,
    redirect_uri: Option<String>,
    response_type: Option<String>,
    scope: Option<String>,
    state: Option<String>,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
    resource: Option<String>,
}

#[derive(serde::Serialize)]
struct LoginContext<'a> {
    #[serde(flatten)]
    shell: context::BrandingShell<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    redirect_encoded: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    passkey_authorize_url: Option<String>,
}

fn passkey_authorize_url(params: &LoginParams) -> Option<String> {
    let client_id = params.client_id.as_ref()?;
    let redirect_uri = params.redirect_uri.as_deref()?;
    let mut pairs = vec![
        (
            "response_type",
            params.response_type.as_deref().unwrap_or("code").to_owned(),
        ),
        ("client_id", client_id.as_str().to_owned()),
        ("redirect_uri", redirect_uri.to_owned()),
    ];
    let optional = [
        ("scope", params.scope.as_deref()),
        ("state", params.state.as_deref()),
        ("code_challenge", params.code_challenge.as_deref()),
        (
            "code_challenge_method",
            params.code_challenge_method.as_deref(),
        ),
        ("resource", params.resource.as_deref()),
    ];
    for (key, value) in optional {
        if let Some(value) = value {
            pairs.push((key, value.to_owned()));
        }
    }
    let query = pairs
        .iter()
        .map(|(k, v)| format!("{k}={}", urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    Some(format!(
        "/api/v1/core/oauth/authorize?{query}&prompt=passkey"
    ))
}

pub(crate) async fn login_page(
    Extension(engine): Extension<AdminTemplateEngine>,
    Query(params): Query<LoginParams>,
) -> AdminHtmlResult<Response> {
    render_login(&engine, &params, "login")
}

// Why: passkey sign-in, kept for platform operators.
pub(crate) async fn operator_login_page(
    Extension(engine): Extension<AdminTemplateEngine>,
    Query(params): Query<LoginParams>,
) -> AdminHtmlResult<Response> {
    render_login(&engine, &params, "login-operator")
}

fn render_login(
    engine: &AdminTemplateEngine,
    params: &LoginParams,
    template: &str,
) -> AdminHtmlResult<Response> {
    let redirect_encoded = sanitize_login_redirect(params.redirect.as_deref())
        .map(|target| urlencoding::encode(&target).into_owned());

    let ctx = LoginContext {
        shell: branding_context(engine),
        redirect_encoded,
        passkey_authorize_url: passkey_authorize_url(params),
    };
    let html = engine.render(template, &ctx)?;
    Ok(Html(html).into_response())
}

fn sanitize_login_redirect(raw: Option<&str>) -> Option<String> {
    let raw = raw?.trim();
    (raw.starts_with('/') && !raw.starts_with("//")).then(|| raw.to_owned())
}
