//! The three approval tools: the pending queue, one decision, and the
//! decided history.
//!
//! These are typed tools rather than CLI passthrough because the approvals
//! dashboard reads them as data. `approval_requests` is core's table, so every
//! query goes through core's `ApprovalRepository` — duplicating the schema in
//! an extension is how two readers of one table start disagreeing.

use rmcp::ErrorData as McpError;
use systemprompt::database::DbPool;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext;
use systemprompt::security::policy::{ApprovalRepository, ApprovalStatus, ApprovalVerdict};

use super::approval_shape::{decided_row, decided_table, pending_row, pending_table};
use crate::tools::{
    ApprovalDecideInput, ApprovalHistoryInput, ApprovalListInput, TOOL_APPROVAL_DECIDE,
    TOOL_APPROVAL_HISTORY, TOOL_APPROVAL_LIST,
};

const DEFAULT_LIMIT: i64 = 25;
const MAX_LIMIT: i64 = 200;

fn resolve_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
}

fn repository(db_pool: &DbPool) -> Result<ApprovalRepository, McpError> {
    let pool = db_pool
        .write_pool_arc()
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
    Ok(ApprovalRepository::new((*pool).clone()))
}

// Why: taking `impl Display` rather than `&sqlx::Error` keeps the driver out
// of this crate's dependencies. Nothing else here touches the database
// directly — every query goes through core's `ApprovalRepository` — so naming
// the concrete error type would be the only reason to link sqlx at all.
fn db_error(e: &impl std::fmt::Display) -> McpError {
    McpError::internal_error(format!("approval_requests query failed: {e}"), None)
}

#[derive(Debug)]
pub struct ApprovalListHandler {
    pub db_pool: DbPool,
}

impl McpToolHandler for ApprovalListHandler {
    type Input = ApprovalListInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_APPROVAL_LIST
    }

    fn description(&self) -> &'static str {
        "List the tool calls currently held for a human decision."
    }

    async fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(Self::Output, String), McpError> {
        let requests = repository(&self.db_pool)?
            .list_pending(resolve_limit(input.limit))
            .await
            .map_err(|e| db_error(&e))?;

        let rows: Vec<_> = requests.iter().map(pending_row).collect();
        // Why: the table always ships, empty or not — a dashboard that sometimes
        // gets no table has to guess whether the queue is clear or the call
        // failed. Emptiness is said in the summary instead.
        let summary = if rows.is_empty() {
            "No calls are held for approval.".to_owned()
        } else {
            format!("{} call(s) held for approval", rows.len())
        };
        Ok((CliArtifact::table(pending_table(&rows)), summary))
    }
}

#[derive(Debug)]
pub struct ApprovalDecideHandler {
    pub db_pool: DbPool,
}

impl McpToolHandler for ApprovalDecideHandler {
    type Input = ApprovalDecideInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_APPROVAL_DECIDE
    }

    fn description(&self) -> &'static str {
        "Approve or deny one held tool call, stamping the caller as approver."
    }

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(Self::Output, String), McpError> {
        let approver_id = ctx.user_id().clone();
        // Why: the username is display only, and the id is the identity that
        // matters — so a context without a resolved user record stamps the id
        // rather than refusing or inventing a name.
        let approver_username = ctx
            .user
            .as_ref()
            .map_or_else(|| approver_id.as_str().to_owned(), |u| u.username.clone());
        let status = if input.approve {
            ApprovalStatus::Approved
        } else {
            ApprovalStatus::Denied
        };
        let note = input.note.as_deref().map(str::trim).filter(|n| !n.is_empty());

        let decided = repository(&self.db_pool)?
            .resolve(
                &input.call_id,
                &ApprovalVerdict {
                    status,
                    approver_id: &approver_id,
                    approver_username: &approver_username,
                    note,
                },
            )
            .await
            .map_err(|e| db_error(&e))?;

        let (rows, summary) = match decided.as_ref() {
            Some(req) => (
                vec![decided_row(req)],
                format!("{} is now {}.", req.call_id, req.status),
            ),
            // Why: a call that will not resolve is one that was already
            // decided, expired, or never existed — all three are the same
            // answer to the approver ("your decision changed nothing"), and
            // none of them is an internal error.
            None => (
                Vec::new(),
                format!(
                    "{} could not be decided — it is no longer pending (already decided, expired, \
                     or unknown).",
                    input.call_id
                ),
            ),
        };
        Ok((
            CliArtifact::table(decided_table(&rows, "Approval Decision")),
            summary,
        ))
    }
}

#[derive(Debug)]
pub struct ApprovalHistoryHandler {
    pub db_pool: DbPool,
}

impl McpToolHandler for ApprovalHistoryHandler {
    type Input = ApprovalHistoryInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_APPROVAL_HISTORY
    }

    fn description(&self) -> &'static str {
        "List recently decided approval requests, with who decided each one."
    }

    async fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(Self::Output, String), McpError> {
        let requests = repository(&self.db_pool)?
            .list_decided(resolve_limit(input.limit))
            .await
            .map_err(|e| db_error(&e))?;

        let rows: Vec<_> = requests.iter().map(decided_row).collect();
        let summary = if rows.is_empty() {
            "No approval request has been decided yet.".to_owned()
        } else {
            format!("{} decided approval request(s)", rows.len())
        };
        Ok((
            CliArtifact::table(decided_table(&rows, "Decided Approvals")),
            summary,
        ))
    }
}
