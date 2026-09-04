//! Typed rows and table columns for the chatter tools — `mail.message`.
//!
//! No I/O, so every function here is directly assertable from the external
//! test workspace.
//!
//! The one shaping decision worth naming: `body` arrives from Odoo as HTML and
//! leaves here as plain text. Stripping it at the typed boundary means every
//! consumer — the model's text rendering and a dashboard's row alike — reads
//! the same string, and no consumer has to know that Odoo stores markup.

use systemprompt::models::artifacts::{Column, ColumnType, SortOrder, TableArtifact, TableHints};

use crate::text::html_to_text;

pub(crate) use crate::shape as odoo;

/// One `mail.message`.
///
/// `model` and `res_id` are the anchor: they are what `note_add` needs to reply
/// onto the same record, and what makes a search hit navigable rather than
/// text someone found once and cannot get back to. `record_name` is the
/// display name for that anchor and may be absent; the anchor itself is not.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NoteRow {
    pub id: i64,
    #[serde(deserialize_with = "odoo::text", default)]
    pub model: Option<String>,
    #[serde(deserialize_with = "odoo::integer", default)]
    pub res_id: Option<i64>,
    #[serde(deserialize_with = "odoo::text", default)]
    pub record_name: Option<String>,
    #[serde(deserialize_with = "odoo::many2one", default)]
    pub author_id: Option<String>,
    #[serde(deserialize_with = "odoo::text", default)]
    pub date: Option<String>,
    #[serde(deserialize_with = "odoo::text", default)]
    pub message_type: Option<String>,
    // Why: Plain text. Odoo stores HTML; [`note_rows`] strips it here so that no
    // consumer downstream has to.
    #[serde(deserialize_with = "odoo::text", default)]
    pub body: Option<String>,
}

#[doc(hidden)]
#[must_use]
pub fn note_rows(records: &[serde_json::Value]) -> Vec<NoteRow> {
    // JSON: protocol boundary — records arrive as the RPC client's JSON. A
    // record that fails to type is logged and dropped rather than shipped
    // half-parsed.
    records
        .iter()
        .filter_map(
            |record| match serde_json::from_value::<NoteRow>(record.clone()) {
                Ok(mut row) => {
                    row.body = row.body.as_deref().map(html_to_text).filter(|b| !b.is_empty());
                    Some(row)
                },
                Err(e) => {
                    tracing::warn!(error = %e, "mail.message record did not match NoteRow; dropping");
                    None
                },
            },
        )
        .collect()
}

fn items(rows: &[NoteRow]) -> Vec<serde_json::Value> {
    // JSON: protocol boundary — TableArtifact carries rows as JSON values.
    rows.iter()
        .filter_map(|row| match serde_json::to_value(row) {
            Ok(item) => Some(item),
            Err(e) => {
                tracing::warn!(error = %e, note_id = row.id, "note row did not serialise; dropping");
                None
            },
        })
        .collect()
}

// Why: The cross-record view: every hit needs its anchor, so `model` and
// `res_id` are columns rather than hidden payload.
#[doc(hidden)]
#[must_use]
pub fn note_search_table(rows: &[NoteRow]) -> TableArtifact {
    let columns = vec![
        Column::new("record_name", ColumnType::String).with_header("Record"),
        Column::new("model", ColumnType::String).with_header("Model"),
        Column::new("author_id", ColumnType::String).with_header("Author"),
        Column::new("date", ColumnType::String).with_header("When"),
        Column::new("body", ColumnType::String).with_header("Note"),
    ];
    TableArtifact::new(columns)
        .with_title("Odoo Chatter")
        .with_rows(items(rows))
        .with_hints(
            TableHints::new()
                .with_page_size(8)
                .filterable()
                .with_sortable(vec!["date".to_owned(), "record_name".to_owned()])
                .with_default_sort("date".to_owned(), SortOrder::Desc),
        )
}

// Why: One record's thread: the anchor is the question the caller already
// answered, so the columns are the conversation itself.
#[doc(hidden)]
#[must_use]
pub fn note_thread_table(rows: &[NoteRow]) -> TableArtifact {
    let columns = vec![
        Column::new("date", ColumnType::String).with_header("When"),
        Column::new("author_id", ColumnType::String).with_header("Author"),
        Column::new("message_type", ColumnType::String).with_header("Kind"),
        Column::new("body", ColumnType::String).with_header("Note"),
    ];
    TableArtifact::new(columns)
        .with_title("Odoo Chatter")
        .with_rows(items(rows))
        .with_hints(
            TableHints::new()
                .with_page_size(8)
                .with_sortable(vec!["date".to_owned()])
                .with_default_sort("date".to_owned(), SortOrder::Desc),
        )
}
