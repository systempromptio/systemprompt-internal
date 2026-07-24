//! SSR demo trace: what the governed coding agent actually did, in order.
//!
//! The decisions ledger answers "what has policy denied lately" across the
//! whole deployment. This page answers a narrower question that a demo needs:
//! for THIS agent session, show the prompt gate, the tool gate, the provider
//! calls, and the tool fires as one timeline, so a denial and the model call it
//! prevented sit next to each other rather than in two different tables.

use std::sync::Arc;

use axum::extract::{Extension, Query, State};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use systemprompt::identifiers::{AgentId, SessionId};

use crate::error::{AdminError, AdminHtmlResult};
use crate::repositories::governance::demo_trace;
use crate::templates::AdminTemplateEngine;
use crate::types::{MarketplaceContext, UserContext};

/// The agent id the Pi governance extension sends on every hook event
/// (`examples/pi/extensions/governance.ts`).
const PI_AGENT_ID: &str = "pi_agent";
const SESSION_LIMIT: i64 = 25;
const TRACE_LIMIT: i64 = 300;

#[derive(Debug, Deserialize)]
pub(crate) struct DemoTraceQuery {
    session: Option<String>,
}

#[derive(Debug, Serialize)]
struct SessionView {
    session_id: SessionId,
    allowed: i64,
    denied: i64,
    started_at: String,
    last_at: String,
    url: String,
    is_active: bool,
}

#[derive(Debug, Serialize)]
struct TraceRowView {
    at: String,
    kind: String,
    kind_label: String,
    subject: String,
    outcome: String,
    policy: String,
    has_policy: bool,
    detail: String,
    is_deny: bool,
    is_request: bool,
}

#[derive(Debug, Serialize)]
struct DemoTraceContext {
    page: &'static str,
    title: &'static str,
    hero_title: &'static str,
    hero_subtitle: &'static str,
    sessions: Vec<SessionView>,
    has_sessions: bool,
    session_id: SessionId,
    rows: Vec<TraceRowView>,
    has_rows: bool,
    prompts_blocked: usize,
    tools_blocked: usize,
    model_calls: usize,
}

fn kind_label(kind: &str) -> &'static str {
    match kind {
        "prompt" => "Prompt gate",
        "tool" => "Tool gate",
        "request" => "Model call",
        _ => "Tool fire",
    }
}

fn to_session_views(
    sessions: Vec<demo_trace::DemoSessionRow>,
    selected: Option<&SessionId>,
) -> Vec<SessionView> {
    sessions
        .into_iter()
        .map(|s| SessionView {
            is_active: selected == Some(&s.session_id),
            url: format!("/admin/demo/trace?session={}", s.session_id),
            started_at: s.started_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            last_at: s.last_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            session_id: s.session_id,
            allowed: s.allowed,
            denied: s.denied,
        })
        .collect()
}

fn to_row_views(rows: Vec<demo_trace::DemoTraceRow>) -> Vec<TraceRowView> {
    rows.into_iter()
        .map(|r| TraceRowView {
            at: r.at.format("%H:%M:%S").to_string(),
            kind_label: kind_label(&r.kind).to_owned(),
            is_deny: r.outcome == "deny",
            is_request: r.kind == "request",
            has_policy: !r.policy.is_empty(),
            kind: r.kind,
            subject: r.subject,
            outcome: r.outcome,
            policy: r.policy,
            detail: r.detail,
        })
        .collect()
}

pub(crate) async fn demo_trace_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
    Query(params): Query<DemoTraceQuery>,
) -> AdminHtmlResult<Response> {
    if !user_ctx.is_admin {
        return Err(AdminError::Forbidden("Admin access required.".to_owned()).into());
    }

    let sessions = demo_trace::list_demo_sessions(&pool, &AgentId::new(PI_AGENT_ID), SESSION_LIMIT)
        .await
        .map_err(AdminError::from)?;

    // Default to the most recent session so the page is useful with no query
    // string — the demo run you just finished is the one you want to see.
    let selected = params
        .session
        .filter(|s| !s.is_empty())
        .map(SessionId::new)
        .or_else(|| sessions.first().map(|s| s.session_id.clone()));

    let rows = match selected.as_ref() {
        Some(session) => demo_trace::list_demo_trace(&pool, session, TRACE_LIMIT)
            .await
            .map_err(AdminError::from)?,
        None => Vec::new(),
    };

    let prompts_blocked = rows
        .iter()
        .filter(|r| r.kind == "prompt" && r.outcome == "deny")
        .count();
    let tools_blocked = rows
        .iter()
        .filter(|r| r.kind == "tool" && r.outcome == "deny")
        .count();
    let model_calls = rows.iter().filter(|r| r.kind == "request").count();

    let session_views = to_session_views(sessions, selected.as_ref());
    let row_views = to_row_views(rows);

    let ctx = DemoTraceContext {
        page: "demo-trace",
        title: "Demo Trace",
        hero_title: "Demo Trace",
        hero_subtitle: "Prompt gate, tool gate, model calls, and tool fires for one agent \
                        session, in the order they happened.",
        has_sessions: !session_views.is_empty(),
        sessions: session_views,
        session_id: selected.unwrap_or_else(|| SessionId::new(String::new())),
        has_rows: !row_views.is_empty(),
        rows: row_views,
        prompts_blocked,
        tools_blocked,
        model_calls,
    };

    Ok(super::render_typed_page(
        &engine,
        "demo-trace",
        &ctx,
        &user_ctx,
        &mkt_ctx,
    ))
}
