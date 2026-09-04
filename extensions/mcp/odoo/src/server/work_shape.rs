//! Typed rows and table columns for the work tools — `mail.activity`,
//! `project.task` and `calendar.event`.
//!
//! The same split [`super::crm_shape`] and [`super::sales_shape`] make: no
//! I/O here, so every function is directly assertable from the external test
//! workspace.
//!
//! Why these exist at all: a dashboard consumes rows, not prose. These tools
//! used to answer with markdown only, which left the artifact regexing the
//! rendering back apart — and a regex over a display string silently returns
//! the wrong thing the day a renderer changes. The markdown row builders stay
//! for the model-facing text; machines read the typed `items`.

use systemprompt::models::artifacts::{Column, ColumnType, SortOrder, TableArtifact, TableHints};

pub(crate) use crate::shape as odoo;

/// One `mail.activity` as `search_read` returns it.
///
/// `id` is the handle `activity_complete` needs, which is why it is required
/// rather than optional: a row that cannot be acted on is not worth shipping.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActivityRow {
    pub id: i64,
    #[serde(deserialize_with = "odoo::text", default)]
    pub summary: Option<String>,
    #[serde(deserialize_with = "odoo::many2one", default)]
    pub activity_type_id: Option<String>,
    #[serde(deserialize_with = "odoo::text", default)]
    pub res_model: Option<String>,
    #[serde(deserialize_with = "odoo::text", default)]
    pub res_name: Option<String>,
    #[serde(deserialize_with = "odoo::text", default)]
    pub date_deadline: Option<String>,
    #[serde(rename = "user_id", deserialize_with = "odoo::many2one", default)]
    pub assignee: Option<String>,
}

/// One `project.task`.
///
/// `assignees` is a count rather than the id list Odoo ships: the question a
/// reader asks of a task list is "is anyone on this?", and `task_update` takes
/// `user_ids` wholesale when the answer is no.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskRow {
    pub id: i64,
    #[serde(deserialize_with = "odoo::text", default)]
    pub name: Option<String>,
    #[serde(deserialize_with = "odoo::many2one", default)]
    pub project_id: Option<String>,
    #[serde(deserialize_with = "odoo::many2one", default)]
    pub stage_id: Option<String>,
    #[serde(deserialize_with = "odoo::text", default)]
    pub date_deadline: Option<String>,
    #[serde(deserialize_with = "odoo::text", default)]
    pub priority: Option<String>,
    #[serde(rename = "user_ids", deserialize_with = "odoo::many2many_ids", default)]
    pub assignee_ids: Vec<i64>,
}

/// One `calendar.event`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EventRow {
    pub id: i64,
    #[serde(deserialize_with = "odoo::text", default)]
    pub name: Option<String>,
    #[serde(deserialize_with = "odoo::text", default)]
    pub start: Option<String>,
    #[serde(deserialize_with = "odoo::text", default)]
    pub stop: Option<String>,
    #[serde(deserialize_with = "odoo::text", default)]
    pub location: Option<String>,
    #[serde(deserialize_with = "odoo::text", default)]
    pub description: Option<String>,
    #[serde(
        rename = "partner_ids",
        deserialize_with = "odoo::many2many_ids",
        default
    )]
    pub attendee_ids: Vec<i64>,
}

// Why: a record that fails to type is logged and dropped rather than shipped
// half-parsed — the same contract [`super::crm_shape::lead_rows`] keeps.
macro_rules! typed_rows {
    ($name:ident, $row:ty, $what:literal) => {
        #[doc(hidden)]
        #[must_use]
        pub fn $name(records: &[serde_json::Value]) -> Vec<$row> {
            // JSON: protocol boundary — records arrive as the RPC client's JSON.
            records
                .iter()
                .filter_map(|record| match serde_json::from_value::<$row>(record.clone()) {
                    Ok(row) => Some(row),
                    Err(e) => {
                        tracing::warn!(error = %e, concat!($what, " record did not match its row type; dropping"));
                        None
                    },
                })
                .collect()
        }
    };
}

typed_rows!(activity_rows, ActivityRow, "mail.activity");
typed_rows!(task_rows, TaskRow, "project.task");
typed_rows!(event_rows, EventRow, "calendar.event");

// Why: TableArtifact carries rows as JSON values, so serialising the typed row
// is the one place the shape crosses back out. A row that will not serialise is
// dropped with its id named, never shipped empty.
fn items<T: serde::Serialize>(rows: &[T], id_of: impl Fn(&T) -> i64) -> Vec<serde_json::Value> {
    rows.iter()
        .filter_map(|row| match serde_json::to_value(row) {
            Ok(item) => Some(item),
            Err(e) => {
                tracing::warn!(error = %e, id = id_of(row), "row did not serialise; dropping");
                None
            },
        })
        .collect()
}

#[doc(hidden)]
#[must_use]
pub fn activity_table(rows: &[ActivityRow]) -> TableArtifact {
    let columns = vec![
        Column::new("id", ColumnType::Integer),
        Column::new("summary", ColumnType::String).with_header("Activity"),
        Column::new("activity_type_id", ColumnType::String).with_header("Type"),
        Column::new("res_name", ColumnType::String).with_header("On record"),
        Column::new("date_deadline", ColumnType::Date).with_header("Due"),
    ];
    TableArtifact::new(columns)
        .with_title("Odoo Activities")
        .with_rows(items(rows, |r| r.id))
        .with_hints(
            TableHints::new()
                .with_page_size(8)
                .filterable()
                .with_sortable(vec!["date_deadline".to_owned(), "summary".to_owned()])
                .with_default_sort("date_deadline".to_owned(), SortOrder::Asc),
        )
}

#[doc(hidden)]
#[must_use]
pub fn task_table(rows: &[TaskRow]) -> TableArtifact {
    let columns = vec![
        Column::new("id", ColumnType::Integer),
        Column::new("name", ColumnType::String).with_header("Task"),
        Column::new("project_id", ColumnType::String).with_header("Project"),
        Column::new("stage_id", ColumnType::String).with_header("Stage"),
        Column::new("date_deadline", ColumnType::Date).with_header("Due"),
    ];
    TableArtifact::new(columns)
        .with_title("Odoo Tasks")
        .with_rows(items(rows, |r| r.id))
        .with_hints(
            TableHints::new()
                .with_page_size(8)
                .filterable()
                .with_sortable(vec!["date_deadline".to_owned(), "name".to_owned()])
                .with_default_sort("date_deadline".to_owned(), SortOrder::Asc),
        )
}

#[doc(hidden)]
#[must_use]
pub fn event_table(rows: &[EventRow]) -> TableArtifact {
    let columns = vec![
        Column::new("id", ColumnType::Integer),
        Column::new("name", ColumnType::String).with_header("Event"),
        Column::new("start", ColumnType::String).with_header("Starts"),
        Column::new("location", ColumnType::String).with_header("Where"),
    ];
    TableArtifact::new(columns)
        .with_title("Odoo Calendar")
        .with_rows(items(rows, |r| r.id))
        .with_hints(
            TableHints::new()
                .with_page_size(8)
                .with_sortable(vec!["start".to_owned()])
                .with_default_sort("start".to_owned(), SortOrder::Asc),
        )
}
