//! Shared persistence helpers for MCP extension crates.
//!
//! Both functions exposed here ([`record_mcp_access`] and
//! [`record_mcp_access_rejected`]) are best-effort: they log a `tracing::warn!`
//! and return on any DB failure, so an MCP request that has already cleared
//! authz is never blocked by an audit-row insert. Callers do not need to
//! propagate errors.

use serde::Serialize;
use systemprompt::database::DbPool;
use systemprompt::identifiers::UserId;

mod repositories;

/// Audit-row metadata persisted to `user_activity.metadata` for every MCP
/// access event. `reason` is present only on rejections.
#[derive(Debug, Serialize)]
pub struct AuditMetadata {
    pub tool_name: String,
    pub server: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

use repositories::McpAccessParams;

use repositories::find_anonymous_user_id;

use repositories::insert_mcp_access;

use repositories::insert_mcp_access_rejection;

const ACTION_USED: &str = "used";

/// Reduce a tool result to the plain `CallToolResult` shape every MCP client
/// validates: a single text block, no `structuredContent`, no `_meta`, no
/// embedded `ui://` resource.
///
/// The rich shape core's response builder emits is consumed by the gateway
/// chat surface, but strict hosts — the Claude Cowork artifact bridge among
/// them — reject it wholesale, and every artifact then shows a validation
/// error instead of data. In the rich shape `content[0]` is only a one-line
/// summary; the markdown body rides inside `structuredContent.artifact.content`,
/// so the body is promoted into the text block before the envelope is
/// dropped. Artifact persistence is unaffected: the structured output is in
/// Postgres before this runs, and the `ui://` resource stays resolvable via
/// `resources/read`. Set `MCP_PLAIN_RESULTS=0` to restore the rich wire
/// shape (it must also be in the server's `env_vars` passthrough allowlist).
#[must_use]
pub fn plain_wire_result(mut result: rmcp::model::CallToolResult) -> rmcp::model::CallToolResult {
    let plain = std::env::var("MCP_PLAIN_RESULTS")
        .map_or(true, |v| !matches!(v.trim(), "0" | "false" | "off"));
    if !plain {
        return result;
    }

    let body = result
        .structured_content
        .as_ref()
        .and_then(|sc| sc.pointer("/artifact/content"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let summary = result
        .content
        .iter()
        .find_map(|block| block.as_text().map(|t| t.text.clone()));

    let text = match (summary, body) {
        (Some(s), Some(b)) if s != b => format!("{s}\n\n{b}"),
        (_, Some(b)) => b,
        (Some(s), None) => s,
        (None, None) => String::new(),
    };

    result.content = vec![rmcp::model::ContentBlock::text(text)];
    result.structured_content = None;
    result.meta = None;
    result.result_type = None;
    result
}

/// Maximum length (in bytes) of the reason text kept in a rejection
/// description before it is truncated. Truncated text gains a "..." suffix, so
/// the reason portion never exceeds `MAX_REASON_LEN + 3` bytes.
#[doc(hidden)]
pub const MAX_REASON_LEN: usize = 117;

/// Truncate `s` to at most `max_bytes`, appending "..." when truncation
/// occurred. The cut is snapped down to a UTF-8 char boundary so it can never
/// split a multi-byte codepoint (which would panic on a byte slice).
///
/// Exposed (behind `#[doc(hidden)]`) so the external test workspace can assert
/// the char-boundary and "..." suffix semantics directly; not part of the
/// public API.
#[doc(hidden)]
pub fn truncate_on_char_boundary(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_owned();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &s[..end])
}

pub async fn record_mcp_access(
    pool: &DbPool,
    user_id: &UserId,
    server: &str,
    tool: &str,
    action: &str,
) {
    let Some(pg_pool) = pool.pool() else {
        tracing::warn!("No PgPool available to record MCP access event");
        return;
    };
    let description = match action {
        "authenticated" => format!("Authenticated to {server} for '{tool}'"),
        ACTION_USED => format!("Executed '{tool}' on {server}"),
        _ => format!("{action} on {server}"),
    };
    let entity_type = if action == ACTION_USED {
        "tool"
    } else {
        "mcp_server"
    };
    let entity_name = if action == ACTION_USED { tool } else { server };
    let metadata = AuditMetadata {
        tool_name: tool.to_owned(),
        server: server.to_owned(),
        reason: None,
    };

    let params = McpAccessParams {
        user_id,
        action,
        entity_type,
        entity_name,
        description: &description,
        metadata: &metadata,
    };

    if let Err(e) = insert_mcp_access(pg_pool.as_ref(), &params).await {
        tracing::warn!(error = %e, "Failed to record MCP access event (non-fatal)");
    }
}

pub async fn record_mcp_access_rejected(pool: &DbPool, server: &str, tool: &str, reason: &str) {
    let Some(pg_pool) = pool.pool() else {
        tracing::warn!("No PgPool available to record MCP access rejection");
        return;
    };
    let reason_text = truncate_on_char_boundary(reason, MAX_REASON_LEN);
    let description = format!("Access rejected on {server}: {reason_text}");
    let metadata = AuditMetadata {
        tool_name: tool.to_owned(),
        server: server.to_owned(),
        reason: Some(reason.to_owned()),
    };

    let anonymous_user_id = match find_anonymous_user_id(pg_pool.as_ref()).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            tracing::error!(
                server,
                tool,
                "Dropping MCP access-rejection audit row: no anonymous principal exists to \
                 attribute it to (refusing to attribute a rejection to an arbitrary user)"
            );
            return;
        },
        Err(e) => {
            tracing::error!(error = %e, server, tool, "Failed to resolve anonymous principal for MCP access-rejection audit; dropping row");
            return;
        },
    };

    if let Err(e) = insert_mcp_access_rejection(
        pg_pool.as_ref(),
        &anonymous_user_id,
        server,
        &description,
        &metadata,
    )
    .await
    {
        tracing::warn!(error = %e, "Failed to record MCP access rejection (non-fatal)");
    }
}
