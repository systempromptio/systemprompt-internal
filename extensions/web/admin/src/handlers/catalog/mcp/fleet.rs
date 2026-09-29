//! The fleet: every declared MCP server plus every name the runtime served
//! under, joined to what the runtime knows about each.
//!
//! Assembled here once for the list page and the fleet export alike, so the
//! CSV can never disagree with the table it was downloaded from.

use sqlx::PgPool;

use crate::error::AdminResult;
use crate::handlers::shared;
use crate::repositories::mcp::runtime;
use crate::types::ENTITY_MCP_SERVER;
use crate::util::time_range::TimeRange;

use super::super::view::assignment_counts_by_type;
use super::rows::{self, RowInputs, Runtime};
use super::view::McpServerRow;

// Why: both reads are best-effort. A runtime table that will not answer
// must not take the page down with it — the declaration alone is still worth
// rendering, and the status column then says "Idle" rather than inventing a
// liveness it could not read.
pub(super) async fn load_runtime(pool: &PgPool, range: TimeRange) -> Runtime {
    let connections = runtime::list_mcp_connections(pool)
        .await
        .inspect_err(|e| tracing::warn!(error = %e, "mcp: connection read failed"))
        .unwrap_or_default();
    let activity = runtime::list_mcp_server_activity(pool, range.from, range.to)
        .await
        .inspect_err(|e| tracing::warn!(error = %e, "mcp: activity read failed"))
        .unwrap_or_default();
    Runtime::new(connections, activity)
}

pub(crate) struct Fleet {
    pub rows: Vec<McpServerRow>,
    pub builtin_calls: i64,
}

// Why: the fleet as one list — every declared server plus every name the
// runtime served under — assembled once for the page and the export alike, so
// the CSV can never disagree with the table it was downloaded from.
pub(crate) async fn fleet_rows(
    pool: &PgPool,
    roles: &[String],
    range: TimeRange,
) -> AdminResult<Fleet> {
    let path = shared::get_services_path()?;
    let catalog = super::super::data::load_catalog(&path, roles);
    let counts = assignment_counts_by_type(pool, ENTITY_MCP_SERVER).await;
    let rt = load_runtime(pool, range).await;

    let mut ids: Vec<String> = catalog
        .mcp
        .iter()
        .map(|s| s.id.as_str().to_owned())
        .collect();
    for name in rt.names() {
        if !ids.contains(&name) {
            ids.push(name);
        }
    }

    let rows = ids
        .iter()
        .map(|id| {
            let server = catalog.mcp.iter().find(|s| s.id.as_str() == id);
            rows::build_row(&RowInputs {
                id,
                server,
                runtime: &rt,
                plugin_count: catalog.plugins_by_mcp.get(id).map_or(0, Vec::len),
                assignment_count: counts.get(id).copied().unwrap_or(0),
            })
        })
        .collect();
    Ok(Fleet {
        rows,
        builtin_calls: rt.builtin_calls,
    })
}
