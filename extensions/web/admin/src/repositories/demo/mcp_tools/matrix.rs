//! User × MCP tool matrix, folded from the same invocation rows as the table.

use sqlx::PgPool;

use super::invocations::list_mcp_tool_invocations;
use crate::repositories::demo::filter::DemoFilter;
use crate::repositories::demo::skill_matrix::{MatrixEntry, UsageMatrix, fold_matrix};

// Why: lint-ok: repository-naming — see `skill_matrix::list_user_skill_matrix`;
// UsageMatrix is a row list plus its column header.
pub async fn list_user_mcp_tool_matrix(
    pool: &PgPool,
    filter: &DemoFilter,
) -> Result<UsageMatrix, sqlx::Error> {
    let entries = list_mcp_tool_invocations(pool, filter)
        .await?
        .into_iter()
        .map(|inv| MatrixEntry {
            user_id: inv.user_id,
            user_email: inv.user_email,
            column: inv.tool_name,
            total_tokens: inv.total_tokens,
            cost_microdollars: inv.cost_microdollars,
        })
        .collect::<Vec<_>>();
    Ok(fold_matrix(&entries))
}
