//! SSR page driving first-run setup, for admins and users alike.
//!
//! This is one of only three admin routes a non-admin can reach (see
//! `middleware::gates::is_non_admin_allowed_path`), so it is the whole of a
//! salesperson's in-app onboarding. Every phase must therefore describe
//! something this instance actually ships, link somewhere that actually
//! resolves, and read a distinct piece of state for its completion — a phase
//! whose `complete` restates the previous phase's is a check that cannot fail.

use std::sync::Arc;

use crate::error::AdminHtmlResult;
use crate::templates::AdminTemplateEngine;
use crate::types::{MarketplaceContext, UserContext};
use axum::extract::{Extension, Query, State};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Debug, Serialize)]
struct SetupPageContext {
    page: &'static str,
    title: &'static str,
    phases: Vec<SetupPhase>,
    all_phases_started: bool,
    just_verified: bool,
}

#[derive(Debug, Serialize)]
struct SetupPhase {
    number: u8,
    title: String,
    description: String,
    guide_url: &'static str,
    action_url: &'static str,
    action_label: &'static str,
    complete: bool,
    current: bool,
}

#[derive(Deserialize, Debug)]
pub(crate) struct SetupQuery {
    #[serde(default)]
    verified: Option<String>,
}

// Why: each field is a distinct signal, so no phase can restate another's.
struct SetupState {
    odoo_linked: bool,
    device_enrolled: bool,
    gateway_used: bool,
}

async fn read_setup_state(pool: &PgPool, user_ctx: &UserContext) -> SetupState {
    let odoo_linked = crate::repositories::users::odoo_identity::find(pool, &user_ctx.user_id)
        .await
        .ok()
        .flatten()
        .is_some();

    // Why: an enrolled device holds a live API key; a revoked one is not enrolled.
    let device_enrolled =
        crate::repositories::bridge::list_api_keys_for_user(pool, &user_ctx.user_id)
            .await
            .map(|keys| keys.iter().any(|k| k.revoked_at.is_none()))
            .unwrap_or(false);

    // Why: a request through the gateway is the only proof the client is wired
    // up and the skills are reachable — nothing else observes the other end.
    let gateway_used =
        crate::repositories::users::usage::get_usage_window(pool, &user_ctx.user_id, 30)
            .await
            .map(|w| w.requests > 0)
            .unwrap_or(false);

    SetupState {
        odoo_linked,
        device_enrolled,
        gateway_used,
    }
}

fn build_phases(user_ctx: &UserContext, state: &SetupState) -> Vec<SetupPhase> {
    // Why: admins hold a second, separate setup skill; a user does not, and
    // must never be told to look for one that is not in their grant.
    let run_setup_description = if user_ctx.is_admin {
        "In your Claude client, run the Systemprompt Setup skill. It reports what your account was granted, then installs your workspace dashboards. As an admin, follow it with Systemprompt Setup — Control Plane for the user, activity, and cost dashboards."
    } else {
        "In your Claude client, run the Systemprompt Setup skill. It reports what your account was granted, checks your connections, and installs your dashboards."
    };

    vec![
        SetupPhase {
            number: 1,
            title: String::from("Sign in with a passkey"),
            description: String::from(
                "Done — you are signed in. There is no signup form and no password: your passkey is the credential, and every action you take is attributed to this account.",
            ),
            guide_url: "/documentation/authentication",
            action_url: "/admin/profile",
            action_label: "View Profile",
            complete: true,
            current: false,
        },
        SetupPhase {
            number: 2,
            title: String::from("Link your Odoo account"),
            description: String::from(
                "Odoo is the system of record, and every call runs as you — the server holds no service account. Add your Odoo login and personal API key on your profile; until you do, every CRM tool returns a clear error naming this page.",
            ),
            guide_url: "/documentation/odoo",
            action_url: "/admin/profile",
            action_label: "Link Odoo",
            complete: state.odoo_linked,
            current: !state.odoo_linked,
        },
        SetupPhase {
            number: 3,
            title: String::from("Install the desktop bridge"),
            description: String::from(
                "The bridge points your Claude client at this instance and syncs the skills, servers, and dashboards your account was granted. Enrol this machine from your profile to get its key.",
            ),
            guide_url: "/documentation/connect-claude-code",
            action_url: "/admin/profile",
            action_label: "Enrol Device",
            complete: state.device_enrolled,
            current: state.odoo_linked && !state.device_enrolled,
        },
        SetupPhase {
            number: 4,
            title: String::from("Run setup in your client"),
            description: String::from(run_setup_description),
            guide_url: "/documentation/",
            action_url: "/skills/",
            action_label: "Browse Skills",
            complete: state.gateway_used,
            current: state.device_enrolled && !state.gateway_used,
        },
    ]
}

pub(crate) async fn setup_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
    Query(query): Query<SetupQuery>,
) -> AdminHtmlResult<Response> {
    let state = read_setup_state(&pool, &user_ctx).await;
    let phases = build_phases(&user_ctx, &state);

    let ctx = SetupPageContext {
        page: "setup",
        title: "Setup Guide",
        all_phases_started: state.odoo_linked,
        just_verified: query.verified.is_some(),
        phases,
    };

    Ok(super::render_typed_page(
        &engine, "setup", &ctx, &user_ctx, &mkt_ctx,
    ))
}
