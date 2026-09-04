//! The skill-by-user usage matrix and its per-user totals.
//!
//! A plane of its own: the matrix is the only view here built from a
//! cross-tabulation rather than from a row, and it carries its own scaling
//! rules for the bar cells.

use serde::Serialize;

use super::view::{describe_user, format_demo_cost, split_qualified};
use crate::handlers::ssr::format::format_token_total;
use crate::repositories::demo::{UsageMatrix, UsageMatrixRow};

#[derive(Debug, Serialize)]
pub(super) struct UserTotalView {
    pub user_email: String,
    pub total: i64,
    pub tokens_display: String,
    pub cost_display: String,
}

#[derive(Debug, Serialize)]
pub(super) struct MatrixCellView {
    pub count: i64,
    pub pct: i64,
    pub is_zero: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct MatrixRowView {
    pub user_email: String,
    pub cells: Vec<MatrixCellView>,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub(super) struct MatrixColumnView {
    pub label: String,
    pub full: String,
}

#[derive(Debug, Serialize)]
pub(super) struct MatrixView {
    pub columns: Vec<MatrixColumnView>,
    pub rows: Vec<MatrixRowView>,
    pub has_data: bool,
}

pub(super) fn matrix_view(matrix: &UsageMatrix) -> MatrixView {
    let max = matrix
        .rows
        .iter()
        .flat_map(|r| r.cells.iter().copied())
        .max()
        .unwrap_or(0);
    MatrixView {
        columns: matrix
            .columns
            .iter()
            .map(String::as_str)
            .map(matrix_column_view)
            .collect(),
        rows: matrix
            .rows
            .iter()
            .map(|r| matrix_row_view(r, max))
            .collect(),
        has_data: !matrix.columns.is_empty() && !matrix.rows.is_empty(),
    }
}

// Why: the header shows the bare name and carries the qualified id as its
// title. A column per `plugin:skill` string is what made this table wider than
// the page it sits in, and the qualifier repeats down the whole header row.
fn matrix_column_view(column: &str) -> MatrixColumnView {
    let (_, name) = split_qualified(column);
    MatrixColumnView {
        label: name,
        full: column.to_owned(),
    }
}

fn matrix_row_view(row: &UsageMatrixRow, max: i64) -> MatrixRowView {
    MatrixRowView {
        user_email: describe_user(row.user_email.as_ref(), row.user_id.as_str()),
        cells: row
            .cells
            .iter()
            .map(|&count| MatrixCellView {
                count,
                pct: crate::handlers::ssr::types::bar_pct(count, max),
                is_zero: count == 0,
            })
            .collect(),
        total: row.total,
    }
}

pub(super) fn user_total_views(matrix: &UsageMatrix) -> Vec<UserTotalView> {
    matrix
        .rows
        .iter()
        .map(|r| UserTotalView {
            user_email: describe_user(r.user_email.as_ref(), r.user_id.as_str()),
            total: r.total,
            tokens_display: format_token_total(r.total_tokens),
            cost_display: format_demo_cost(r.cost_microdollars),
        })
        .collect()
}
