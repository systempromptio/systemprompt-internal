//! Everything the device-link pages are drawn from.
//!
//! Why this is split off: the handler beside it is a consent flow with real
//! decisions in it — which account is linked, whether a redirect is safe to
//! honour. The template contexts, the strings shown to the operator, and the
//! canned refusal responses are none of those. Separating them keeps the
//! decisions readable and stops a wording change from touching the same file
//! as the credential-issuing path.

use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use serde::Serialize;
use sqlx::PgPool;
use systemprompt_web_shared::BrandingConfig;
use systemprompt_web_shared::html_escape;

use crate::error::{AdminHtmlError, AdminHtmlResult};
use crate::repositories::users::federated::list_federated_identities_for_user;
use crate::templates::AdminTemplateEngine;

// Why: unconfigured branding must stay a missing key rather than a null, so
// the template's `{{#if}}` guard behaves. `redirect`/`redirect_host` follow the
// same rule: absent, not empty, when there is no callback to return to.
// Why the account fields are DB-sourced and the session field is not: they
// answer different questions. `account_*` is who the durable token will belong
// to, which only the `users` row can say. `session_email` is what the browser's
// JWT claims, which is what the operator *thinks* they are. When those disagree
// the page has to show both — silently preferring either one is how a consent
// screen ends up naming somebody the token is not for.
#[derive(Debug, Serialize)]
pub(super) struct DeviceLinkContext<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) branding: Option<&'a BrandingConfig>,
    pub(super) account_email: String,
    pub(super) account_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) session_email: Option<String>,
    pub(super) email_mismatch: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) signed_in_via: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) redirect: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) redirect_host: Option<String>,
    pub(super) code_ttl_seconds: i64,
    pub(super) switch_account_href: String,
}

// Why: rendered when the page cannot state whose token this would be.
#[derive(Debug, Serialize)]
pub(super) struct DeviceLinkBlockedContext<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) branding: Option<&'a BrandingConfig>,
    pub(super) blocked_reason: String,
    pub(super) switch_account_href: String,
}

#[derive(Debug, Serialize)]
pub(super) struct DeviceCodeContext<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) branding: Option<&'a BrandingConfig>,
    pub(super) approved: bool,
    pub(super) account_email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) code: Option<String>,
    pub(super) expires_in_seconds: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) login_command: Option<String>,
}

pub(super) fn render_blocked(
    engine: &AdminTemplateEngine,
    branding: Option<&BrandingConfig>,
    switch_account_href: String,
    reason: &str,
) -> AdminHtmlResult<Response> {
    let data = DeviceLinkBlockedContext {
        branding,
        blocked_reason: reason.to_owned(),
        switch_account_href,
    };
    let data = serde_json::to_value(&data).map_err(AdminHtmlError::internal)?;
    let html = engine.render("bridge-device-link-blocked", &data)?;
    // Why: this is a rendered consent page carrying the refusal, not an error
    // response. Routing it through AdminError would discard the branded body
    // telling the operator why the link was blocked. lint-ok: http-error
    Ok((StatusCode::FORBIDDEN, Html(html)).into_response())
}

// Why: describes how the current session authenticated, for the operator to check
// against what they just did.
//
// Best-effort: this is context, not a gate, so a lookup failure degrades to
// showing nothing rather than blocking a legitimate authorisation.
pub(super) async fn describe_sign_in(
    pool: &PgPool,
    user_id: &systemprompt::identifiers::UserId,
) -> Option<String> {
    let identities = list_federated_identities_for_user(pool, user_id)
        .await
        .ok()?;
    if identities.is_empty() {
        return None;
    }

    let odoo_login = crate::repositories::users::odoo_identity::find(pool, user_id)
        .await
        .ok()
        .flatten()
        .map(|oi| oi.odoo_login);

    let described: Vec<String> = identities
        .iter()
        .map(|fi| {
            let provider = describe_issuer(&fi.issuer);
            match odoo_login.as_deref() {
                Some(login) if fi.issuer.starts_with("odoo:") => {
                    format!("{provider} as {login}")
                },
                _ => provider,
            }
        })
        .collect();

    Some(described.join(", "))
}

// Why: turns a stored issuer string into something an operator recognises.
//
// Odoo issuers are `odoo:{url}/{db}`; the URL is the part that tells someone
// *which* Odoo they just authenticated against, which is the whole point.
pub(super) fn describe_issuer(issuer: &str) -> String {
    issuer.strip_prefix("odoo:").map_or_else(
        || issuer.to_owned(),
        |rest| {
            let host = rest
                .split_once("://")
                .map_or(rest, |(_, after)| after)
                .split('/')
                .next()
                .unwrap_or(rest);
            format!("Odoo ({host})")
        },
    )
}

pub(super) fn render_code_page(
    engine: &AdminTemplateEngine,
    ctx: &DeviceCodeContext<'_>,
) -> AdminHtmlResult<Response> {
    let data = serde_json::to_value(ctx).map_err(AdminHtmlError::internal)?;
    let html = engine.render("bridge-device-code", &data)?;
    Ok(Html(html).into_response())
}

// Why: `--gateway` is only worth printing when the server knows its own
// external URL; a wrong one is worse than the CLI's configured default.
pub(super) fn gateway_suffix() -> String {
    systemprompt::models::Config::get().map_or_else(
        |_| String::new(),
        |c| format!(" --gateway {}", c.api_external_url.trim_end_matches('/')),
    )
}

pub(super) fn validate_loopback_redirect(redirect: &str) -> Option<String> {
    let url = url::Url::parse(redirect).ok()?;
    if url.scheme() != "http" {
        return None;
    }
    let host = url.host_str()?;
    if host != "127.0.0.1" && host != "localhost" {
        return None;
    }
    let port = url.port()?;
    Some(format!("{host}:{port}"))
}

// Why: refuses an approval whose account differs from the one consented to.
pub(super) fn consent_conflict_response(message: &str) -> Response {
    // Why: lint-ok: http-error — states what was and was not issued, which the
    // generic page cannot
    (
        StatusCode::CONFLICT,
        Html(format!(
            "<h1>Account changed</h1><p>{}</p><p><a href=\"/bridge-auth/device-link\">Start \
             over</a></p>",
            html_escape(message)
        )),
    )
        .into_response()
}

pub(super) fn bad_redirect_response(redirect: &str) -> Response {
    // Why: lint-ok: http-error — names the accepted redirect forms, which the
    // generic page cannot
    tracing::warn!(
        redirect,
        "Rejected bridge device-link redirect (non-loopback)"
    );
    (
        StatusCode::BAD_REQUEST,
        Html(format!(
            "<h1>Invalid redirect</h1><p>Only http://127.0.0.1:PORT or http://localhost:PORT redirects are accepted. Got: <code>{}</code></p>",
            html_escape(redirect)
        )),
    )
        .into_response()
}
