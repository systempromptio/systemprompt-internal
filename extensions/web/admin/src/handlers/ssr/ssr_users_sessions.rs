//! `/admin/entities/sessions` — the live agent-session board.
//!
//! Every row is a session that reported a hook event inside the liveness
//! window. `handle` is the address a teammate or agent uses to reach it;
//! `workspace` and `git_branch` say which repository the work is landing in.

use crate::error::{AdminError, AdminHtmlResult};
use crate::handlers::ssr::format::format_cost;
use crate::repositories::analytics::live_sessions::{
    DEFAULT_LIVE_WINDOW_MINUTES, LiveSessionRow, WorkspaceCostRow, list_live_sessions,
    list_workspace_costs,
};
use crate::templates::AdminTemplateEngine;
use crate::types::{MarketplaceContext, UserContext};
use axum::extract::{Extension, State};
use axum::response::Response;
use serde::Serialize;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt::identifiers::{SessionId, UserId};

const LIVE_SESSION_LIMIT: i64 = 200;
const WORKSPACE_COST_LIMIT: i64 = 25;
const WORKSPACE_COST_DAYS: i64 = 30;

#[derive(Debug, Serialize)]
struct LiveSessionView {
    session_id: SessionId,
    detail_url: String,
    user_id: UserId,
    display_name: String,
    handle: String,
    workspace: String,
    git_branch: String,
    cwd: String,
    model: String,
    current_activity: String,
    client_source: String,
    permission_mode: String,
    tool_uses: i64,
    prompts: i64,
    errors: i64,
    cost: String,
    context_pct: Option<i16>,
    idle_for: String,
}

#[derive(Debug, Serialize)]
struct WorkspaceCostView {
    workspace: String,
    session_count: i64,
    request_count: i64,
    cost: String,
    input_tokens: i64,
    output_tokens: i64,
}

#[derive(Debug, Serialize)]
struct UsersSessionsContext {
    page: &'static str,
    title: &'static str,
    cli_command: &'static str,
    cli_command_list: &'static str,
    window_minutes: i32,
    session_count: usize,
    sessions: Vec<LiveSessionView>,
    cost_days: i64,
    workspaces: Vec<WorkspaceCostView>,
}

fn dash_if_empty(value: Option<String>) -> String {
    value
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "—".to_owned())
}

fn format_idle(last_event_at: Option<chrono::DateTime<chrono::Utc>>) -> String {
    let Some(seen) = last_event_at else {
        return "—".to_owned();
    };
    let seconds = (chrono::Utc::now() - seen).num_seconds().max(0);
    if seconds < 60 {
        return "just now".to_owned();
    }
    let minutes = seconds / 60;
    format!("{minutes}m ago")
}

fn to_view(row: LiveSessionRow) -> LiveSessionView {
    LiveSessionView {
        detail_url: format!("/admin/entities/sessions/{}", row.session_id),
        idle_for: format_idle(row.last_event_at),
        cost: format_cost(row.live_cost_microdollars.unwrap_or(0)),
        display_name: dash_if_empty(row.display_name),
        handle: dash_if_empty(row.handle),
        workspace: dash_if_empty(row.workspace),
        git_branch: dash_if_empty(row.git_branch),
        cwd: dash_if_empty(row.cwd),
        model: dash_if_empty(row.model),
        current_activity: dash_if_empty(row.current_activity),
        client_source: dash_if_empty(row.client_source),
        permission_mode: dash_if_empty(row.permission_mode),
        tool_uses: row.tool_uses.unwrap_or(0),
        prompts: row.prompts.unwrap_or(0),
        errors: row.errors.unwrap_or(0),
        context_pct: row.context_pct,
        session_id: row.session_id,
        user_id: row.user_id,
    }
}

fn to_cost_view(row: WorkspaceCostRow) -> WorkspaceCostView {
    WorkspaceCostView {
        workspace: row.workspace.unwrap_or_else(|| "—".to_owned()),
        session_count: row.session_count.unwrap_or(0),
        request_count: row.request_count.unwrap_or(0),
        cost: format_cost(row.total_cost_microdollars.unwrap_or(0)),
        input_tokens: row.total_input_tokens.unwrap_or(0),
        output_tokens: row.total_output_tokens.unwrap_or(0),
    }
}

pub(crate) async fn users_sessions_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
) -> AdminHtmlResult<Response> {
    if !user_ctx.is_admin {
        return Err(AdminError::Forbidden("Admin access required.".to_owned()).into());
    }

    let rows = list_live_sessions(&pool, DEFAULT_LIVE_WINDOW_MINUTES, LIVE_SESSION_LIMIT).await?;
    let sessions: Vec<LiveSessionView> = rows.into_iter().map(to_view).collect();

    let since = chrono::Utc::now() - chrono::Duration::days(WORKSPACE_COST_DAYS);
    let workspaces: Vec<WorkspaceCostView> =
        list_workspace_costs(&pool, since, WORKSPACE_COST_LIMIT)
            .await?
            .into_iter()
            .map(to_cost_view)
            .collect();

    let ctx = UsersSessionsContext {
        page: "sessions",
        title: "Live Sessions",
        cli_command: "systemprompt admin session show",
        cli_command_list: "systemprompt admin session list",
        window_minutes: DEFAULT_LIVE_WINDOW_MINUTES,
        session_count: sessions.len(),
        sessions,
        cost_days: WORKSPACE_COST_DAYS,
        workspaces,
    };
    Ok(super::render_typed_page(
        &engine,
        "users-sessions",
        &ctx,
        &user_ctx,
        &mkt_ctx,
    ))
}
