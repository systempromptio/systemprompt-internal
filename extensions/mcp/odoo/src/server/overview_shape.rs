//! The briefing, shaped as a dashboard.
//!
//! One section per block of [`super::briefing::Briefing`], each carrying the
//! typed rows the single-model tools already ship — a lead in the briefing is
//! the same `LeadRow` `crm_lead_search` returns, so a dashboard reading both
//! reads one shape.
//!
//! No I/O, so every function here is directly assertable from the external
//! test workspace.

use systemprompt::models::artifacts::dashboard::TextSectionData;
use systemprompt::models::artifacts::{
    DashboardArtifact, DashboardHints, DashboardSection, LayoutMode, MetricCard, MetricsCardsData,
    SectionType, TableSectionData,
};

use super::briefing::{Briefing, RECENT_DAYS, TASK_HORIZON_DAYS};
use super::crm_shape::lead_rows;
use super::notes_shape::note_rows;
use super::work_shape::{activity_rows, event_rows, task_rows};

pub(crate) use crate::shape as odoo;

// Why: One `read_group` bucket of the pipeline.
//
// Not a record: `__count` and the summed revenue exist only in the
// aggregate, which is why the pipeline block is metric cards rather than a
// seventh row type nothing else would ever return.
#[derive(Debug, Clone, serde::Deserialize)]
struct PipelineGroup {
    #[serde(deserialize_with = "odoo::many2one", default)]
    stage_id: Option<String>,
    #[serde(rename = "__count", deserialize_with = "odoo::integer", default)]
    count: Option<i64>,
    #[serde(deserialize_with = "odoo::number", default)]
    expected_revenue: Option<f64>,
}

fn pipeline_cards(records: &[serde_json::Value]) -> Vec<MetricCard> {
    // JSON: protocol boundary — buckets arrive as the RPC client's JSON.
    records
        .iter()
        .filter_map(|record| match serde_json::from_value::<PipelineGroup>(record.clone()) {
            Ok(group) => Some(group),
            Err(e) => {
                tracing::warn!(error = %e, "pipeline bucket did not match PipelineGroup; dropping");
                None
            },
        })
        .map(|group| {
            MetricCard::new(
                group.stage_id.unwrap_or_else(|| "Unstaged".to_owned()),
                format!("{} lead(s)", group.count.unwrap_or_default()),
            )
            .with_subtitle(format!(
                "Expected revenue {:.2}",
                group.expected_revenue.unwrap_or_default()
            ))
        })
        .collect()
}

// Why: a quiet block still gets its section — a dashboard that drops the
// calendar on a day with no meetings reads as a broken query. The block names
// its own empty case instead.
fn empty_section(
    id: &str,
    title: impl Into<String>,
    empty: &str,
    order: u32,
) -> Result<DashboardSection, serde_json::Error> {
    DashboardSection::new(id, title, SectionType::Text)
        .with_data(TextSectionData::new(empty))
        .map(|section| section.with_order(order))
}

fn table_section<T: serde::Serialize>(
    id: &str,
    title: impl Into<String>,
    columns: &[&str],
    rows: &[T],
    order: u32,
) -> Result<DashboardSection, serde_json::Error> {
    let items = rows
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()?;
    let columns = columns.iter().map(|c| (*c).to_owned()).collect();
    DashboardSection::new(id, title, SectionType::Table)
        .with_data(TableSectionData::new(columns, items))
        .map(|section| section.with_order(order))
}

// Why: the five fields that describe a block travel together everywhere — one
// struct keeps `rows` as the only free argument and stops the call sites
// reading as six positional strings whose order nothing checks.
struct Block<'a> {
    id: &'a str,
    title: String,
    columns: &'a [&'a str],
    empty: &'a str,
    order: u32,
}

fn rows_section<T: serde::Serialize>(
    block: Block<'_>,
    rows: &[T],
) -> Result<DashboardSection, serde_json::Error> {
    let Block {
        id,
        title,
        columns,
        empty,
        order,
    } = block;
    if rows.is_empty() {
        empty_section(id, title, empty, order)
    } else {
        table_section(id, title, columns, rows, order)
    }
}

// Why: the pipeline block is the one section that is not a table — it comes
// from `read_group`, so it carries counts and sums rather than records. Kept
// separate so `briefing_dashboard` reads as the list of blocks it builds.
fn pipeline_section(briefing: &Briefing) -> Result<DashboardSection, serde_json::Error> {
    let cards = pipeline_cards(&briefing.pipeline);
    if cards.is_empty() {
        return empty_section(
            "pipeline",
            "Pipeline by stage",
            "No leads in the pipeline.",
            0,
        );
    }
    Ok(
        DashboardSection::new("pipeline", "Pipeline by stage", SectionType::MetricsCards)
            .with_data(MetricsCardsData::new(cards))?
            .with_order(0),
    )
}

pub(crate) fn briefing_dashboard(
    briefing: &Briefing,
) -> Result<DashboardArtifact, serde_json::Error> {
    let sections = vec![
        pipeline_section(briefing)?,
        rows_section(
            Block {
                id: "new_leads",
                title: format!("Leads created in the last {RECENT_DAYS} days"),
                columns: &[
                    "id",
                    "name",
                    "stage_id",
                    "user_id",
                    "partner_name",
                    "expected_revenue",
                ],
                empty: "No new leads this week.",
                order: 1,
            },
            &lead_rows(&briefing.new_leads),
        )?,
        rows_section(
            Block {
                id: "activities",
                title: "Your activities, overdue and due today".to_owned(),
                columns: &[
                    "id",
                    "summary",
                    "activity_type_id",
                    "res_name",
                    "date_deadline",
                ],
                empty: "Nothing due — your activity list is clear.",
                order: 2,
            },
            &activity_rows(&briefing.activities),
        )?,
        rows_section(
            Block {
                id: "calendar",
                title: "Today's calendar".to_owned(),
                columns: &["id", "name", "start", "location"],
                empty: "Nothing in the calendar today.",
                order: 3,
            },
            &event_rows(&briefing.events),
        )?,
        rows_section(
            Block {
                id: "tasks",
                title: format!("Open tasks due in the next {TASK_HORIZON_DAYS} days"),
                columns: &["id", "name", "project_id", "stage_id", "date_deadline"],
                empty: "No tasks fall due this week.",
                order: 4,
            },
            &task_rows(&briefing.tasks),
        )?,
        rows_section(
            Block {
                id: "notes",
                title: "Recent notes".to_owned(),
                columns: &["date", "record_name", "model", "author_id", "body"],
                empty: "No recent chatter.",
                order: 5,
            },
            &note_rows(&briefing.notes),
        )?,
    ];

    Ok(DashboardArtifact::new("Odoo Business Overview")
        .with_sections(sections)
        .with_hints(DashboardHints::new().with_layout(LayoutMode::Vertical)))
}
