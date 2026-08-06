//! Domains and payloads for the scheduling and collaboration tools.
//!
//! These tools write to other people's days, so the assertions concentrate on
//! the places a silent mistake costs someone real time: a task list that
//! quietly includes the archive, an event whose end is before its start, a
//! project name that matches nothing and gets created anyway.

use systemprompt_mcp_odoo::server::calendar::{
    DEFAULT_DURATION_HOURS, event_domain, event_values, normalize_datetime,
};
use systemprompt_mcp_odoo::server::channels::{channel_domain, channel_row};
use systemprompt_mcp_odoo::server::tasks::{task_domain, task_row};
use systemprompt_mcp_odoo::tools::inputs::{
    CalendarEventCreateInput, CalendarEventListInput, TaskListInput,
};

fn event(start: &str, stop: Option<&str>, hours: Option<f64>) -> CalendarEventCreateInput {
    CalendarEventCreateInput {
        name: "Kickoff".to_owned(),
        start: start.to_owned(),
        stop: stop.map(str::to_owned),
        duration_hours: hours,
        attendee_partner_ids: None,
        description: None,
        model: None,
        res_id: None,
    }
}

fn tasks(project: Option<&str>, query: Option<&str>, open_only: Option<bool>) -> TaskListInput {
    TaskListInput {
        project: project.map(str::to_owned),
        query: query.map(str::to_owned),
        open_only,
        limit: None,
    }
}

#[test]
fn iso_datetimes_are_rewritten_into_odoos_form() {
    assert_eq!(normalize_datetime("2026-08-07T10:00:00"), "2026-08-07 10:00:00");
    assert_eq!(normalize_datetime("2026-08-07T10:00:00Z"), "2026-08-07 10:00:00");
    assert_eq!(
        normalize_datetime("2026-08-07T10:00:00.123Z"),
        "2026-08-07 10:00:00",
        "Odoo's ORM rejects fractional seconds"
    );
}

#[test]
fn a_datetime_already_in_odoos_form_is_untouched() {
    assert_eq!(normalize_datetime("2026-08-07 10:00:00"), "2026-08-07 10:00:00");
}

#[test]
fn only_the_date_time_separator_is_replaced() {
    assert_eq!(
        normalize_datetime("2026-08-07T10:00:00"),
        "2026-08-07 10:00:00",
        "a later T, were there one, must not also become a space"
    );
}

#[test]
fn an_event_with_no_end_gets_the_default_duration() {
    let values = event_values(&event("2026-08-07T10:00:00", None, None));

    assert_eq!(values["start"], "2026-08-07 10:00:00");
    assert_eq!(
        values["stop"], "2026-08-07 11:00:00",
        "{DEFAULT_DURATION_HOURS} hour is the default, and Odoo requires a stop"
    );
}

#[test]
fn a_duration_in_hours_sets_the_end() {
    let values = event_values(&event("2026-08-07T09:00:00", None, Some(2.5)));

    assert_eq!(values["stop"], "2026-08-07 11:30:00", "fractional hours count");
}

#[test]
fn an_explicit_end_beats_a_duration() {
    let values = event_values(&event(
        "2026-08-07T09:00:00",
        Some("2026-08-07T17:00:00"),
        Some(1.0),
    ));

    assert_eq!(values["stop"], "2026-08-07 17:00:00");
}

#[test]
fn attendees_use_odoos_set_replacement_command() {
    let mut input = event("2026-08-07 09:00:00", None, None);
    input.attendee_partner_ids = Some(vec![3, 9]);

    let values = event_values(&input);

    assert_eq!(
        values["partner_ids"],
        serde_json::json!([[6, 0, [3, 9]]]),
        "a bare list of ids is not a valid x2many write"
    );
}

#[test]
fn a_record_link_is_written_only_when_both_halves_are_present() {
    let mut linked = event("2026-08-07 09:00:00", None, None);
    linked.model = Some("crm.lead".to_owned());
    linked.res_id = Some(42);
    let values = event_values(&linked);
    assert_eq!(values["res_model"], "crm.lead");
    assert_eq!(values["res_id"], 42);

    let mut half = event("2026-08-07 09:00:00", None, None);
    half.res_id = Some(42);
    let values = event_values(&half);
    assert!(
        values.get("res_id").is_none(),
        "an id with no model would attach the event to nothing: {values}"
    );
}

#[test]
fn the_event_window_covers_whole_days() {
    let input = CalendarEventListInput {
        date_from: Some("2026-08-01".to_owned()),
        date_to: Some("2026-08-31".to_owned()),
        query: None,
        limit: None,
    };

    let domain = event_domain(&input);

    assert_eq!(
        domain,
        serde_json::json!([
            ["start", ">=", "2026-08-01 00:00:00"],
            ["start", "<=", "2026-08-31 23:59:59"]
        ]),
        "a bare date as an upper bound would exclude everything on the last day"
    );
}

#[test]
fn task_list_hides_closed_stages_by_default() {
    let domain = task_domain(&tasks(None, None, None), None);

    assert_eq!(
        domain,
        serde_json::json!([["stage_id.fold", "=", false]]),
        "\"what is on my plate\" should not return the archive"
    );
}

#[test]
fn task_list_can_be_asked_for_everything() {
    assert_eq!(
        task_domain(&tasks(None, None, Some(false)), None),
        serde_json::json!([]),
    );
}

#[test]
fn a_resolved_project_becomes_an_id_filter() {
    let domain = task_domain(&tasks(Some("Acme"), Some("migration"), Some(false)), Some(7));

    assert_eq!(
        domain,
        serde_json::json!([
            ["project_id", "=", 7],
            ["name", "ilike", "%migration%"]
        ]),
        "the name was resolved to an id before the domain was built"
    );
}

#[test]
fn a_task_row_names_its_project_and_stage() {
    let record = serde_json::json!({
        "id": 5,
        "name": "Migrate data",
        "project_id": [7, "Acme Rollout"],
        "stage_id": [2, "In Progress"],
        "user_ids": [3],
        "date_deadline": "2026-08-10",
    });

    let row = task_row(&record);

    assert!(row.starts_with("- **[5] Migrate data**"), "got: {row}");
    assert!(row.contains("Acme Rollout") && row.contains("In Progress"));
    assert!(row.contains("2026-08-10") && row.contains("1 assignee"));
}

#[test]
fn channel_search_is_a_name_filter_or_nothing() {
    assert_eq!(channel_domain(None), serde_json::json!([]));
    assert_eq!(channel_domain(Some("   ")), serde_json::json!([]));
    assert_eq!(
        channel_domain(Some("sales")),
        serde_json::json!([["name", "ilike", "%sales%"]])
    );
}

#[test]
fn a_channel_row_leads_with_the_id_channel_post_needs() {
    let record = serde_json::json!({
        "id": 4,
        "name": "sales",
        "channel_type": "channel",
        "member_count": 12,
    });

    let row = channel_row(&record);

    assert!(row.starts_with("- **[4] sales**"), "got: {row}");
    assert!(row.contains("12 member"));
}
