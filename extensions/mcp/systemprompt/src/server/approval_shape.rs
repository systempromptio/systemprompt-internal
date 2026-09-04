//! Typed rows and table columns for the approval tools.
//!
//! The same split the Odoo server makes between shaping and I/O: nothing here
//! touches the database, so every function is directly assertable.
//!
//! Why rows rather than prose: the approvals dashboard consumes rows. A tool
//! that answered in markdown would leave the page regexing its own rendering
//! back apart, and a regex over a display string returns the wrong thing the
//! day the renderer changes.

use systemprompt::models::artifacts::{Column, ColumnType, SortOrder, TableArtifact, TableHints};
use systemprompt::security::policy::ApprovalRequest;

/// One held call, exactly as the approver must see it.
///
/// `arguments` is the tool payload verbatim rather than a summary of it: the
/// approver authorises what will actually run. It stays a `Value` for that
/// reason — re-rendering it would be the summary this field exists to avoid.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PendingRow {
    pub call_id: String,
    pub tool_name: String,
    pub server_name: String,
    pub requested_by: String,
    pub rule: String,
    // JSON: protocol boundary — the held call's own MCP arguments.
    pub arguments: serde_json::Value,
    pub args_digest: String,
    #[serde(rename = "session_id")]
    pub session: Option<String>,
    pub trace_id: Option<String>,
    pub created_at: String,
    pub expires_at: String,
}

/// One decided call, carrying who decided it and when.
///
/// Expired rows land here too: nobody decided them, and the empty approver is
/// the honest record of that.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DecidedRow {
    pub call_id: String,
    pub tool_name: String,
    pub server_name: String,
    pub requested_by: String,
    pub rule: String,
    // JSON: protocol boundary — the held call's own MCP arguments.
    pub arguments: serde_json::Value,
    pub args_digest: String,
    pub status: String,
    pub approver_id: Option<String>,
    pub approver_username: Option<String>,
    pub decided_at: Option<String>,
    pub decision_note: Option<String>,
    pub created_at: String,
}

#[must_use]
pub fn pending_row(req: &ApprovalRequest) -> PendingRow {
    PendingRow {
        call_id: req.call_id.clone(),
        tool_name: req.tool_name.clone(),
        server_name: req.server_name.clone(),
        requested_by: req.requested_by.clone(),
        rule: req.rule.clone(),
        arguments: req.arguments.clone(),
        args_digest: req.args_digest.clone(),
        session: req.session_id.clone(),
        trace_id: req.trace_id.clone(),
        created_at: req.created_at.to_rfc3339(),
        expires_at: req.expires_at.to_rfc3339(),
    }
}

#[must_use]
pub fn decided_row(req: &ApprovalRequest) -> DecidedRow {
    DecidedRow {
        call_id: req.call_id.clone(),
        tool_name: req.tool_name.clone(),
        server_name: req.server_name.clone(),
        requested_by: req.requested_by.clone(),
        rule: req.rule.clone(),
        arguments: req.arguments.clone(),
        args_digest: req.args_digest.clone(),
        status: req.status.as_str().to_owned(),
        approver_id: req.approver_id.clone(),
        approver_username: req.approver_username.clone(),
        decided_at: req.decided_at.map(|d| d.to_rfc3339()),
        decision_note: req.decision_note.clone(),
        created_at: req.created_at.to_rfc3339(),
    }
}

// Why: TableArtifact carries rows as JSON values, so serialising the typed row
// is the one place the shape crosses back out. A row that will not serialise is
// dropped with its call id named, never shipped half-formed.
fn items<T: serde::Serialize>(rows: &[T], id_of: impl Fn(&T) -> &str) -> Vec<serde_json::Value> {
    rows.iter()
        .filter_map(|row| match serde_json::to_value(row) {
            Ok(item) => Some(item),
            Err(e) => {
                tracing::warn!(error = %e, call_id = id_of(row), "approval row did not serialise; dropping");
                None
            },
        })
        .collect()
}

#[must_use]
pub fn pending_table(rows: &[PendingRow]) -> TableArtifact {
    let columns = vec![
        Column::new("call_id", ColumnType::String).with_header("Call"),
        Column::new("tool_name", ColumnType::String).with_header("Tool"),
        Column::new("server_name", ColumnType::String).with_header("Server"),
        Column::new("requested_by", ColumnType::String).with_header("Requested by"),
        Column::new("rule", ColumnType::String).with_header("Rule"),
        Column::new("created_at", ColumnType::String).with_header("Held since"),
        Column::new("expires_at", ColumnType::String).with_header("Expires"),
    ];
    TableArtifact::new(columns)
        .with_title("Held Calls")
        .with_rows(items(rows, |r| r.call_id.as_str()))
        .with_hints(
            TableHints::new()
                .with_page_size(10)
                .filterable()
                .with_sortable(vec!["created_at".to_owned(), "tool_name".to_owned()])
                .with_default_sort("created_at".to_owned(), SortOrder::Desc),
        )
}

#[must_use]
pub fn decided_table(rows: &[DecidedRow], title: &str) -> TableArtifact {
    let columns = vec![
        Column::new("call_id", ColumnType::String).with_header("Call"),
        Column::new("tool_name", ColumnType::String).with_header("Tool"),
        Column::new("status", ColumnType::String).with_header("Decision"),
        Column::new("approver_username", ColumnType::String).with_header("Approver"),
        Column::new("decided_at", ColumnType::String).with_header("Decided"),
        Column::new("decision_note", ColumnType::String).with_header("Note"),
        Column::new("requested_by", ColumnType::String).with_header("Requested by"),
    ];
    TableArtifact::new(columns)
        .with_title(title)
        .with_rows(items(rows, |r| r.call_id.as_str()))
        .with_hints(
            TableHints::new()
                .with_page_size(10)
                .filterable()
                .with_sortable(vec!["decided_at".to_owned(), "tool_name".to_owned()])
                .with_default_sort("decided_at".to_owned(), SortOrder::Desc),
        )
}
