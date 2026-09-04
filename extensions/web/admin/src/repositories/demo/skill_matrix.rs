//! User × entity usage matrix, shared by the skill and MCP tool pages.
//!
//! Columns are ordered by descending total so the busiest entity is leftmost;
//! every row carries a cell per column, zero-filled, so the template can render
//! the grid without looking anything up.

use std::collections::HashMap;

use sqlx::PgPool;
use systemprompt::identifiers::UserId;

use super::filter::DemoFilter;
use super::skill_invocations::list_skill_invocations;

#[derive(Debug, Clone, Default)]
pub struct UsageMatrix {
    pub columns: Vec<String>,
    pub rows: Vec<UsageMatrixRow>,
}

#[derive(Debug, Clone)]
pub struct UsageMatrixRow {
    pub user_id: UserId,
    pub user_email: Option<String>,
    pub cells: Vec<i64>,
    pub total: i64,
    pub total_tokens: i64,
    pub cost_microdollars: i64,
}

#[derive(Debug, Clone)]
pub struct MatrixEntry {
    pub user_id: UserId,
    pub user_email: Option<String>,
    pub column: String,
    pub total_tokens: i64,
    pub cost_microdollars: i64,
}

// Why: lint-ok: repository-naming — UsageMatrix is a list of per-user rows
// wrapped only to carry the column header they are aligned to, so `list_` is
// what the call site reads, not `get_`.
pub async fn list_user_skill_matrix(
    pool: &PgPool,
    filter: &DemoFilter,
) -> Result<UsageMatrix, sqlx::Error> {
    let entries = list_skill_invocations(pool, filter)
        .await?
        .into_iter()
        .map(|inv| MatrixEntry {
            user_id: inv.user_id,
            user_email: inv.user_email,
            column: inv.skill,
            total_tokens: inv.total_tokens,
            cost_microdollars: inv.cost_microdollars,
        })
        .collect::<Vec<_>>();
    Ok(fold_matrix(entries))
}

pub fn fold_matrix(entries: Vec<MatrixEntry>) -> UsageMatrix {
    let mut column_totals: HashMap<String, i64> = HashMap::new();
    for e in &entries {
        *column_totals.entry(e.column.clone()).or_insert(0) += 1;
    }
    let mut columns: Vec<String> = column_totals.keys().cloned().collect();
    columns.sort_by(|a, b| {
        column_totals[b]
            .cmp(&column_totals[a])
            .then_with(|| a.cmp(b))
    });
    let index: HashMap<&str, usize> = columns
        .iter()
        .enumerate()
        .map(|(i, c)| (c.as_str(), i))
        .collect();

    let mut by_user: HashMap<UserId, UsageMatrixRow> = HashMap::new();
    let mut order: Vec<UserId> = Vec::new();
    for e in &entries {
        let row = by_user.entry(e.user_id.clone()).or_insert_with(|| {
            order.push(e.user_id.clone());
            UsageMatrixRow {
                user_id: e.user_id.clone(),
                user_email: e.user_email.clone(),
                cells: vec![0; columns.len()],
                total: 0,
                total_tokens: 0,
                cost_microdollars: 0,
            }
        });
        if let Some(&i) = index.get(e.column.as_str()) {
            row.cells[i] += 1;
        }
        row.total += 1;
        row.total_tokens += e.total_tokens;
        row.cost_microdollars += e.cost_microdollars;
    }

    let mut rows: Vec<UsageMatrixRow> = order
        .into_iter()
        .filter_map(|id| by_user.remove(&id))
        .collect();
    rows.sort_by(|a, b| {
        b.total
            .cmp(&a.total)
            .then_with(|| a.user_email.cmp(&b.user_email))
    });
    UsageMatrix { columns, rows }
}
