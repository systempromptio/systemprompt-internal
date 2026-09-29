//! The reads behind everything under the header on `/admin/mcp/{id}`.
//!
//! Split out of the handler because six loosely related lists and their
//! pagination is most of what the page does, and a handler that also owns the
//! request flow stops being readable at that size. Every windowed read takes
//! the page's range; the connections list is "now", not a window.

use std::sync::Arc;
use systemprompt::identifiers::McpServerId;

use sqlx::PgPool;

use crate::handlers::ssr::list_view::{PageWindow, Pagination};
use crate::repositories::mcp::runtime;
use crate::util::time_range::TimeRange;

use super::rows::BASE_URL;
use super::{detail, view};
use crate::handlers::ssr::list_view::{DEFAULT_PAGE_SIZE, paginate};

const TOOL_LIMIT: i64 = 50;
const CALLER_LIMIT: i64 = 25;
const CONNECTION_LIMIT: i64 = 50;

// Why: the page links carry the window, or turning a page would silently
// reset the log to the default 24 hours.
fn build_pagination(window: PageWindow, base: &str, window_query: &str) -> Pagination {
    let suffix = if window_query.is_empty() {
        String::new()
    } else {
        format!("&{window_query}")
    };
    paginate(window, |page| format!("{base}?page={page}{suffix}"))
}

// Why: everything below the header on the detail page, in one read. It is a
// struct rather than a tuple because seven loosely related lists returned
// positionally is how the wrong one ends up rendered in the wrong section.
pub(super) struct DetailSections {
    pub tools: Vec<view::McpToolRow>,
    pub callers: Vec<view::McpCallerRowView>,
    pub executions: Vec<view::McpExecutionRowView>,
    pub executions_total: i64,
    pub pagination: Pagination,
    pub connections: Vec<view::McpConnectionRowView>,
}

// Why: every read is best-effort for the same reason the fleet reads are — a
// section that will not load renders its own empty state rather than taking
// the page down with it.
pub(super) async fn detail_sections(
    pool: &Arc<PgPool>,
    mcp_id: &McpServerId,
    range: TimeRange,
    page_index: i64,
    window_query: &str,
) -> DetailSections {
    let (executions, executions_total) = runtime::list_mcp_executions_paged(
        pool,
        runtime::CallLogQuery {
            server_name: Some(mcp_id.as_str()),
            from: range.from,
            to: range.to,
            limit: DEFAULT_PAGE_SIZE,
            offset: page_index * DEFAULT_PAGE_SIZE,
        },
    )
    .await
    .inspect_err(|e| tracing::warn!(error = %e, "mcp: execution log read failed"))
    .unwrap_or_default();
    let shown = i64::try_from(executions.len()).unwrap_or(0);

    let tools = runtime::list_mcp_tool_stats(
        pool,
        Some(mcp_id.as_str()),
        range.from,
        range.to,
        TOOL_LIMIT,
    )
    .await
    .inspect_err(|e| tracing::warn!(error = %e, "mcp: tool stats read failed"))
    .unwrap_or_default();
    let callers =
        runtime::list_mcp_callers(pool, mcp_id.as_str(), range.from, range.to, CALLER_LIMIT)
            .await
            .inspect_err(|e| tracing::warn!(error = %e, "mcp: caller read failed"))
            .unwrap_or_default();
    let connections =
        runtime::list_mcp_connections_for_server(pool, mcp_id.as_str(), CONNECTION_LIMIT)
            .await
            .inspect_err(|e| tracing::warn!(error = %e, "mcp: connection read failed"))
            .unwrap_or_default();

    DetailSections {
        pagination: build_pagination(
            PageWindow::new(
                page_index,
                DEFAULT_PAGE_SIZE,
                executions_total,
                shown,
                "calls",
            ),
            &format!("{BASE_URL}/{mcp_id}"),
            window_query,
        ),
        tools: detail::tool_rows(tools),
        callers: detail::caller_rows(callers),
        executions: detail::execution_rows(executions),
        executions_total,
        connections: detail::connection_rows(connections),
    }
}
