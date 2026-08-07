//! Gathering the briefing: six bounded reads, issued together.
//!
//! Split from [`super::overview`] so the rendering and the fetching can be read
//! separately — and because the interesting property lives here. The six
//! queries are independent, so they go out concurrently; a briefing that cost
//! six sequential round trips to Odoo would be the slowest tool on the server,
//! and it is the one most likely to be called first thing in the morning.

use rmcp::ErrorData as McpError;

use crate::error::OdooError;

use super::activity::activity_fields;
use super::call::{OdooCall, lead_fields};
use crate::client::{GroupQuery, SearchOptions};

// Why: how far back "recent" reaches for the new-leads section.
pub const RECENT_DAYS: i64 = 7;
// Why: how far ahead the task section looks. A week is the horizon someone
// planning their day can still act on.
pub const TASK_HORIZON_DAYS: i64 = 7;
// Why: chatter notes included in the briefing.
const RECENT_NOTES: u32 = 20;
// Why: caps the new-leads list, so a busy week cannot crowd out the rest.
const RECENT_LEADS: u32 = 25;
// Why: separate caps for the agenda sections, both small — a day's meetings and
// a week's deadlines are short lists by nature, and a long one is a signal to
// open the calendar rather than to read more here.
const AGENDA_LIMIT: u32 = 15;

const EVENT_FIELDS: [&str; 6] = ["id", "name", "start", "stop", "location", "partner_ids"];

const TASK_FIELDS: [&str; 6] = [
    "id",
    "name",
    "project_id",
    "stage_id",
    "user_ids",
    "date_deadline",
];

// Why: chatter rows carry the record they hang off, so the briefing can say
// which lead a note was about without a second query.
const NOTE_FIELDS: [&str; 7] = [
    "id",
    "subject",
    "record_name",
    "model",
    "res_id",
    "author_id",
    "date",
];

// Why: the four result sets a briefing is assembled from, kept together so
// the fan-out and the rendering agree on what a briefing contains.
#[derive(Debug)]
pub struct Briefing {
    pub pipeline: Vec<serde_json::Value>,
    pub new_leads: Vec<serde_json::Value>,
    pub activities: Vec<serde_json::Value>,
    pub notes: Vec<serde_json::Value>,
    pub events: Vec<serde_json::Value>,
    pub tasks: Vec<serde_json::Value>,
}

// Why: the four queries are independent, so they go out together — a briefing
// that cost four sequential round trips to Odoo would be the slowest tool on
// the server, and it is the one most likely to be called first thing.
pub async fn fetch(call: &OdooCall) -> Result<Briefing, McpError> {
    let now = chrono::Utc::now();
    let today = now.date_naive().to_string();
    let since = (now - chrono::Duration::days(RECENT_DAYS))
        .date_naive()
        .to_string();

    let lead_options = SearchOptions {
        fields: lead_fields(),
        limit: RECENT_LEADS,
        order: Some("create_date desc".to_owned()),
    };
    let activity_options = SearchOptions {
        fields: activity_fields(),
        limit: RECENT_NOTES,
        order: Some("date_deadline asc".to_owned()),
    };
    let note_options = SearchOptions {
        fields: NOTE_FIELDS.iter().map(|f| (*f).to_owned()).collect(),
        limit: RECENT_NOTES,
        order: Some("date desc".to_owned()),
    };

    let pipeline = call.client.read_group(
        &call.creds,
        "crm.lead",
        GroupQuery {
            domain: serde_json::json!([]),
            fields: &["expected_revenue:sum"],
            group_by: &["stage_id"],
        },
    );
    let new_leads = call.client.search_read(
        &call.creds,
        "crm.lead",
        serde_json::json!([["create_date", ">=", since]]),
        &lead_options,
    );
    // Why: `<=` today, not `<` — the briefing is for planning the day, so what
    // is due today belongs beside what is already late.
    let activities = call.client.search_read(
        &call.creds,
        "mail.activity",
        serde_json::json!([
            ["user_id", "=", call.creds.uid],
            ["date_deadline", "<=", today]
        ]),
        &activity_options,
    );
    let notes = call.client.search_read(
        &call.creds,
        "mail.message",
        serde_json::json!([["message_type", "=", "comment"]]),
        &note_options,
    );

    let (pipeline, new_leads, activities, notes, events, tasks) = tokio::try_join!(
        pipeline,
        new_leads,
        activities,
        notes,
        agenda(call, &today),
        due_tasks(call, &today)
    )?;
    Ok(Briefing {
        pipeline,
        new_leads,
        activities,
        notes,
        events,
        tasks,
    })
}

// Why: today only. A briefing that listed the whole week's meetings would bury
// the two that are actually imminent.
async fn agenda(call: &OdooCall, today: &str) -> Result<Vec<serde_json::Value>, OdooError> {
    let options = SearchOptions {
        fields: EVENT_FIELDS.iter().map(|f| (*f).to_owned()).collect(),
        limit: AGENDA_LIMIT,
        order: Some("start asc".to_owned()),
    };
    call.client
        .search_read(
            &call.creds,
            "calendar.event",
            serde_json::json!([
                ["start", ">=", format!("{today} 00:00:00")],
                ["start", "<=", format!("{today} 23:59:59")]
            ]),
            &options,
        )
        .await
}

// Why: open tasks only, and only those with a deadline inside the horizon. An
// undated backlog item is not something today's briefing can act on.
async fn due_tasks(call: &OdooCall, today: &str) -> Result<Vec<serde_json::Value>, OdooError> {
    let horizon = (chrono::Utc::now() + chrono::Duration::days(TASK_HORIZON_DAYS))
        .date_naive()
        .to_string();
    let options = SearchOptions {
        fields: TASK_FIELDS.iter().map(|f| (*f).to_owned()).collect(),
        limit: AGENDA_LIMIT,
        order: Some("date_deadline asc".to_owned()),
    };
    call.client
        .search_read(
            &call.creds,
            "project.task",
            serde_json::json!([
                ["stage_id.fold", "=", false],
                ["date_deadline", "!=", false],
                ["date_deadline", "<=", horizon],
                ["date_deadline", ">=", today]
            ]),
            &options,
        )
        .await
}
