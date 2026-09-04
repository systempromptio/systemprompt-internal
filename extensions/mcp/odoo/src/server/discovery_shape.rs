//! Typed rows and table columns for the discovery tools — the small reference
//! reads a caller makes before a write: `crm_stage_list` today.
//!
//! Split from [`super::crm_shape`] because a stage is not a lead: the lead
//! shaping is the search domain, the tag join and the row, and bolting the
//! pipeline's own vocabulary onto it made one file answer two questions.
//! No I/O, so every function here is directly assertable from the external
//! test workspace.

use systemprompt::models::artifacts::{Column, ColumnType, TableArtifact};

pub(crate) use crate::shape as odoo;

/// One `crm.stage`.
///
/// `is_won` is the flag, not a name match: [`lead_domain`] already refuses to
/// filter "open" by stage name because a rename would break it, and a stage
/// menu that has to guess which entry closes a deal has the same problem.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StageRow {
    pub id: i64,
    #[serde(deserialize_with = "odoo::text", default)]
    pub name: Option<String>,
    #[serde(deserialize_with = "odoo::integer", default)]
    pub sequence: Option<i64>,
    #[serde(default)]
    pub is_won: bool,
    #[serde(default)]
    pub fold: bool,
}

#[doc(hidden)]
#[must_use]
pub fn stage_rows(records: &[serde_json::Value]) -> Vec<StageRow> {
    // JSON: protocol boundary — records arrive as the RPC client's JSON.
    records
        .iter()
        .filter_map(
            |record| match serde_json::from_value::<StageRow>(record.clone()) {
                Ok(row) => Some(row),
                Err(e) => {
                    tracing::warn!(error = %e, "crm.stage record did not match StageRow; dropping");
                    None
                },
            },
        )
        .collect()
}

#[doc(hidden)]
#[must_use]
pub fn stage_table(rows: &[StageRow]) -> TableArtifact {
    let columns = vec![
        Column::new("id", ColumnType::Integer),
        Column::new("name", ColumnType::String).with_header("Stage"),
        Column::new("is_won", ColumnType::Boolean).with_header("Counts as won"),
    ];
    // JSON: protocol boundary — TableArtifact carries rows as JSON values.
    let items = rows
        .iter()
        .filter_map(|row| match serde_json::to_value(row) {
            Ok(item) => Some(item),
            Err(e) => {
                tracing::warn!(error = %e, stage_id = row.id, "stage row did not serialise; dropping");
                None
            },
        })
        .collect();
    // The pipeline is short and already in `sequence` order from the query;
    // re-sorting it in the client would only scramble the reader's mental model
    // of their own funnel.
    TableArtifact::new(columns)
        .with_title("Pipeline Stages")
        .with_rows(items)
}
