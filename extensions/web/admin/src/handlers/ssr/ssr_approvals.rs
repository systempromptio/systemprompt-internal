//! `/admin/governance/approvals` — the queue of tool calls held for a human.
//!
//! The counterpart to the MCP server's approval gate: a call parked by the
//! `require_approval` policy blocks on its `approval_requests` row, and this
//! page is where a person resolves it. The two never speak directly — the row
//! is the whole protocol between them — so the console can be restarted
//! mid-hold without losing a waiting call.
//!
//! Approving is deliberately not a bare id in a URL: both actions are POSTs
//! against the same session-authenticated admin router that guards every other
//! mutation here, so a held call cannot be approved by following a link.
//!
//! Two kinds of row share the table and are listed apart. A *live* hold is a
//! tool call blocking right now, with a fifteen-minute life. An *ingestion
//! proposal* is the brain@ pipeline asking whether an inbound email may become
//! an Odoo record; nobody is blocked on it, it lives for a week, and the
//! `knowledge_odoo_apply` job acts on the answer within a minute. Listing them
//! together would let a week of proposals push a blocking call off the page.

use std::sync::Arc;

use axum::extract::{Extension, Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Serialize;
use sqlx::PgPool;

use systemprompt::security::policy::{
    ApprovalRepository, ApprovalRequest, ApprovalStatus, ApprovalVerdict,
};

use super::ssr_approvals_ingest::{INGESTION_RULE, IngestSummary, humanize, ingest_summary};
use crate::error::{AdminHtmlError, AdminHtmlResult};
use crate::templates::AdminTemplateEngine;
use crate::types::UserContext;

const PAGE: &str = "approvals";
const PAGE_URL: &str = "/admin/governance/approvals";

// Why: a backlog deeper than this is an operational problem to fix, not a
// page to paginate. Proposals accrue for a week, so they get their own budget.
const QUEUE_LIMIT: i64 = 300;

#[derive(Debug, Serialize)]
struct ApprovalsContext {
    page: &'static str,
    title: &'static str,
    live: Vec<PendingRow>,
    ingestion: Vec<PendingRow>,
    live_count: usize,
    ingestion_count: usize,
    has_live: bool,
    has_ingestion: bool,
    is_empty: bool,
}

#[derive(Debug, Serialize)]
struct PendingRow {
    call_id: String,
    tool_name: String,
    server_name: String,
    requested_by: String,
    rule: String,
    // Why: pretty-printed so the approver reads the arguments they are
    // actually authorising, not a one-line blob they will skim past.
    arguments: String,
    trace_id: Option<String>,
    waiting_seconds: i64,
    waiting_human: String,
    expires_in_seconds: i64,
    expires_in_human: String,
    is_ingestion: bool,
    ingest: Option<IngestSummary>,
    approve_url: String,
    deny_url: String,
}

impl PendingRow {
    fn from_request(request: &ApprovalRequest) -> Self {
        let now = chrono::Utc::now();
        let waiting_seconds = (now - request.created_at).num_seconds().max(0);
        let expires_in_seconds = (request.expires_at - now).num_seconds().max(0);
        let ingest = ingest_summary(request);
        Self {
            call_id: request.call_id.clone(),
            tool_name: request.tool_name.clone(),
            server_name: request.server_name.clone(),
            requested_by: request.requested_by.clone(),
            rule: request.rule.clone(),
            arguments: serde_json::to_string_pretty(&request.arguments)
                .unwrap_or_else(|_| request.arguments.to_string()),
            trace_id: request.trace_id.clone(),
            waiting_seconds,
            waiting_human: humanize(waiting_seconds),
            expires_in_seconds,
            expires_in_human: humanize(expires_in_seconds),
            is_ingestion: request.rule == INGESTION_RULE,
            ingest,
            approve_url: format!("{PAGE_URL}/{}/approve", request.call_id),
            deny_url: format!("{PAGE_URL}/{}/deny", request.call_id),
        }
    }
}

pub(crate) async fn approvals_page(
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
) -> AdminHtmlResult<Response> {
    let repo = ApprovalRepository::new((*pool).clone());

    // Why: sweep before listing. An expired row that still says 'pending'
    // would render an Approve button whose click can only fail — the waiter
    // has already given up on it.
    if let Err(err) = repo.expire_due().await {
        tracing::warn!(error = %err, "could not sweep expired approvals before listing");
    }

    let pending = repo
        .list_pending(QUEUE_LIMIT)
        .await
        .map_err(AdminHtmlError::internal)?;

    let (ingestion, live): (Vec<PendingRow>, Vec<PendingRow>) = pending
        .iter()
        .map(PendingRow::from_request)
        .partition(|row| row.is_ingestion);
    let data = ApprovalsContext {
        page: PAGE,
        title: "Pending approvals",
        is_empty: live.is_empty() && ingestion.is_empty(),
        live_count: live.len(),
        ingestion_count: ingestion.len(),
        has_live: !live.is_empty(),
        has_ingestion: !ingestion.is_empty(),
        live,
        ingestion,
    };
    let data = serde_json::to_value(&data).map_err(AdminHtmlError::internal)?;
    Ok(axum::response::Html(engine.render("approvals", &data)?).into_response())
}

pub(crate) async fn approval_approve(
    Extension(user_ctx): Extension<UserContext>,
    State(pool): State<Arc<PgPool>>,
    Path(call_id): Path<String>,
) -> AdminHtmlResult<Response> {
    resolve(&pool, &user_ctx, &call_id, ApprovalStatus::Approved).await
}

pub(crate) async fn approval_deny(
    Extension(user_ctx): Extension<UserContext>,
    State(pool): State<Arc<PgPool>>,
    Path(call_id): Path<String>,
) -> AdminHtmlResult<Response> {
    resolve(&pool, &user_ctx, &call_id, ApprovalStatus::Denied).await
}

async fn resolve(
    pool: &Arc<PgPool>,
    user_ctx: &UserContext,
    call_id: &str,
    status: ApprovalStatus,
) -> AdminHtmlResult<Response> {
    let repo = ApprovalRepository::new((**pool).clone());
    let resolved = repo
        .resolve(
            call_id,
            &ApprovalVerdict {
                status,
                approver_id: &user_ctx.user_id,
                approver_username: &user_ctx.username,
                note: None,
            },
        )
        .await
        .map_err(AdminHtmlError::internal)?;

    if let Some(request) = resolved {
        tracing::info!(
            call_id,
            tool_name = %request.tool_name,
            approver = %user_ctx.username,
            status = %status,
            "held tool call resolved from the admin console"
        );
    } else {
        // Why: not an error page. Two admins racing the same call, or one
        // clicking a call that has just expired, is ordinary; the first
        // decision stands and the queue simply stops listing it.
        tracing::info!(
            call_id,
            approver = %user_ctx.username,
            "approval was already resolved or had expired; leaving the first decision in place"
        );
    }

    Ok(Redirect::to(PAGE_URL).into_response())
}
