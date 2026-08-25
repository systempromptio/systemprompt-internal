//! SSR page listing a user's active sessions.

use crate::error::{AdminError, AdminHtmlResult};
use crate::templates::AdminTemplateEngine;
use crate::types::{MarketplaceContext, UserContext};
use axum::extract::Extension;
use axum::response::Response;
use serde::Serialize;
use systemprompt::identifiers::UserId;

#[derive(Debug, Serialize)]
struct CurrentSessionView {
    user_id: UserId,
    username: String,
    email: String,
    is_admin: bool,
    roles: Vec<String>,
}

#[derive(Debug, Serialize)]
struct UsersSessionsContext {
    page: &'static str,
    title: &'static str,
    cli_command: &'static str,
    cli_command_list: &'static str,
    current_session: CurrentSessionView,
}

pub(crate) async fn users_sessions_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
) -> AdminHtmlResult<Response> {
    if !user_ctx.is_admin {
        return Err(AdminError::Forbidden("Admin access required.".to_owned()).into());
    }

    let current_session = CurrentSessionView {
        user_id: user_ctx.user_id.clone(),
        username: user_ctx.username.clone(),
        email: user_ctx.email.to_string(),
        is_admin: user_ctx.is_admin,
        roles: user_ctx.roles.clone(),
    };

    let ctx = UsersSessionsContext {
        page: "sessions",
        title: "User Sessions",
        cli_command: "systemprompt admin session show",
        cli_command_list: "systemprompt admin session list",
        current_session,
    };
    Ok(super::render_typed_page(
        &engine,
        "users-sessions",
        &ctx,
        &user_ctx,
        &mkt_ctx,
    ))
}
