//! SSR page completing the bridge device-link flow.
//!
//! The redirect target is restricted to loopback: the bridge runs on the user's
//! own machine, and any non-loopback redirect would hand the link code to a
//! third party.
//!
//! `redirect` is optional. A CLI with no browser has nothing listening on
//! loopback, so it sends the user here without one and the approve step
//! *displays* the code for the user to copy back into the terminal.

use std::sync::Arc;

use axum::extract::{Extension, Form, Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::Deserialize;
use sqlx::PgPool;

use crate::error::{AdminHtmlError, AdminHtmlResult};
use crate::repositories::bridge;
use crate::repositories::users::queries::{UserIdentity, find_user_identity};
use crate::services::bridge_profile::BRIDGE_BINARY;
use crate::templates::AdminTemplateEngine;
use crate::types::UserContext;

use super::ssr_bridge_device_link_view::{
    DeviceCodeContext, DeviceLinkContext, bad_redirect_response, consent_conflict_response,
    describe_sign_in, gateway_suffix, render_blocked, render_code_page, validate_loopback_redirect,
};
use super::ssr_helpers::branding_context;

#[derive(Debug, Deserialize)]
pub(crate) struct DeviceLinkQuery {
    pub redirect: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DeviceLinkApproveForm {
    pub redirect: Option<String>,
    // Why: the account the consent page actually displayed, echoed back so approval
    // can be bound to it. Not trusted as identity — only compared against the
    // row re-read at approve time.
    pub confirm_account: Option<String>,
}


pub(crate) async fn device_link_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
    Query(query): Query<DeviceLinkQuery>,
) -> AdminHtmlResult<Response> {
    let redirect_host = match query.redirect.as_deref() {
        Some(redirect) => match validate_loopback_redirect(redirect) {
            Some(host) => Some(host),
            None => return Ok(bad_redirect_response(redirect)),
        },
        None => None,
    };

    let branding = branding_context(&engine).branding;
    let switch_href = switch_account_href(query.redirect.as_deref());

    // Why: read the row instead of trusting the cookie: `UserContext` is decoded
    // from a self-contained JWT, so its email is whatever was true when the token
    // was minted. This page's entire purpose is to let a person recognise the
    // account before a durable credential is issued for it, so the account has to
    // be described by the database, not by the claim.
    let identity = find_user_identity(&pool, &user_ctx.user_id)
        .await
        .map_err(AdminHtmlError::internal)?;

    // Why: fail closed. A consent screen that cannot say whose token this would be
    // must not offer to issue one.
    let Some(UserIdentity {
        email: account_email,
        display_name: account_name,
        is_active: true,
    }) = identity
    else {
        return render_blocked(
            &engine,
            branding,
            switch_href,
            "This session does not resolve to an active account, so no token can be issued for \
             it. Sign in again.",
        );
    };

    let session_email = user_ctx.email.to_string();
    let email_mismatch = !session_email.eq_ignore_ascii_case(&account_email);

    let signed_in_via = describe_sign_in(&pool, &user_ctx.user_id).await;

    let data = DeviceLinkContext {
        branding,
        account_email,
        account_name,
        email_mismatch,
        session_email: email_mismatch.then_some(session_email),
        signed_in_via,
        switch_account_href: switch_href,
        redirect: query.redirect,
        redirect_host,
        code_ttl_seconds: bridge::EXCHANGE_CODE_TTL_SECONDS,
    };
    let data = serde_json::to_value(&data).map_err(AdminHtmlError::internal)?;
    let html = engine.render("bridge-device-link", &data)?;
    Ok(Html(html).into_response())
}


pub(crate) async fn device_link_approve(
    Extension(user_ctx): Extension<UserContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
    Form(form): Form<DeviceLinkApproveForm>,
) -> AdminHtmlResult<Response> {
    if let Some(redirect) = form.redirect.as_deref()
        && validate_loopback_redirect(redirect).is_none()
    {
        return Ok(bad_redirect_response(redirect));
    }

    // Why: re-read here: the page was rendered from one cookie and this POST
    // arrives with whatever cookie the browser holds *now*. Between the two, a
    // session can change — which is exactly the lingering-session hazard this
    // flow is exposed to. Binding the approval to the account the operator was
    // shown means a swap fails loudly instead of minting a durable token for
    // someone they never saw.
    let identity = find_user_identity(&pool, &user_ctx.user_id)
        .await
        .map_err(AdminHtmlError::internal)?;

    let Some(identity) = identity.filter(|i| i.is_active) else {
        return Ok(consent_conflict_response(
            "This session no longer resolves to an active account. Nothing was issued.",
        ));
    };

    if let Some(shown) = form.confirm_account.as_deref()
        && !shown.eq_ignore_ascii_case(&identity.email)
    {
        tracing::warn!(
            user_id = %user_ctx.user_id,
            shown = %shown,
            actual = %identity.email,
            "Bridge device-link approval did not match the account consented to; refusing"
        );
        return Ok(consent_conflict_response(
            "The signed-in account changed after this page was shown. Nothing was issued — \
             reload and check the account before approving.",
        ));
    }

    let issued = bridge::issue_exchange_code(&pool, &user_ctx.user_id).await?;

    let Some(redirect) = form.redirect else {
        let expires_in_seconds = (issued.expires_at - chrono::Utc::now())
            .num_seconds()
            .max(0);
        let login_command = format!(
            "{BRIDGE_BINARY} login --code {code}{gateway}",
            code = issued.code,
            gateway = gateway_suffix()
        );
        return render_code_page(
            &engine,
            &DeviceCodeContext {
                branding: branding_context(&engine).branding,
                approved: true,
                account_email: identity.email.clone(),
                code: Some(issued.code),
                expires_in_seconds,
                login_command: Some(login_command),
            },
        );
    };

    let sep = if redirect.contains('?') { '&' } else { '?' };
    let location = format!("{redirect}{sep}code={}", issued.code);
    Ok(Redirect::to(&location).into_response())
}

pub(crate) async fn device_link_deny(
    Extension(user_ctx): Extension<UserContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    Form(form): Form<DeviceLinkApproveForm>,
) -> AdminHtmlResult<Response> {
    let Some(redirect) = form.redirect else {
        return render_code_page(
            &engine,
            &DeviceCodeContext {
                branding: branding_context(&engine).branding,
                approved: false,
                account_email: user_ctx.email.to_string(),
                code: None,
                expires_in_seconds: 0,
                login_command: None,
            },
        );
    };

    if validate_loopback_redirect(&redirect).is_none() {
        return Ok(bad_redirect_response(&redirect));
    }
    let sep = if redirect.contains('?') { '&' } else { '?' };
    let location = format!("{redirect}{sep}error=denied");
    Ok(Redirect::to(&location).into_response())
}

fn switch_account_href(redirect: Option<&str>) -> String {
    let mut target = "/bridge-auth/device-link".to_owned();
    if let Some(redirect) = redirect {
        target.push_str("?redirect=");
        target.push_str(&urlencoding::encode(redirect));
    }
    format!(
        "/bridge-auth/device-link/switch?next={}",
        urlencoding::encode(&target)
    )
}

// Why: the browser's session cookie decides WHO gets linked, and a machine
// with a lingering session would silently link that account. This route is
// mounted OUTSIDE the auth gate: it clears the session cookies and sends the
// user to the login page with the device-link continuation, so they choose
// the account — Odoo credential or passkey — before anything is approved.
// lint-ok: http-error
pub(crate) async fn device_link_switch(Query(query): Query<SwitchQuery>) -> Response {
    let next = query
        .next
        .filter(|n| n.starts_with('/') && !n.starts_with("//"))
        .unwrap_or_else(|| "/bridge-auth/device-link".to_owned());
    let location = format!("/admin/login?redirect={}", urlencoding::encode(&next));

    let mut response = Redirect::to(&location).into_response();
    // Why: the shared builder: these must match the setter on Path and Secure or
    // the browser keeps the cookie, and this route's whole job is to guarantee the
    // old session is gone before the next screen offers to mint a token.
    let headers = response.headers_mut();
    for (name, value) in &systemprompt_web_shared::session_cookies::clear() {
        headers.append(name, value.clone());
    }
    response
}

#[derive(Debug, Deserialize)]
pub(crate) struct SwitchQuery {
    pub next: Option<String>,
}
