//! `repositories::demo::mcp_tools` — name splitting and failure marking.

use chrono::{Duration, Utc};
use systemprompt_web_admin::repositories::demo::filter::DemoFilter;
use systemprompt_web_admin::repositories::demo::mcp_tools::list_mcp_tool_invocations;

use crate::fixtures::{EventSpec, insert_event, insert_user, unclaimed_email, unique};
use crate::tempdb::TempDb;

#[tokio::test]
async fn the_wire_name_splits_into_server_and_tool() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("u"), &unclaimed_email("demo")).await;
    let session = unique("sess");
    let at = Utc::now() - Duration::minutes(10);

    insert_event(
        &db.pool,
        &EventSpec::mcp_tool(
            &unique("evt"),
            &user,
            &session,
            "mcp__odoo__crm_lead_search",
        )
        .at(at),
    )
    .await;

    let rows = list_mcp_tool_invocations(&db.pool, &DemoFilter::for_user(user))
        .await
        .expect("list mcp invocations");
    let row = rows.first().expect("one invocation");
    assert_eq!(row.server, "odoo");
    assert_eq!(row.tool, "crm_lead_search");
    assert_eq!(row.tool_name, "mcp__odoo__crm_lead_search");
    assert!(!row.is_failure);
}

#[tokio::test]
async fn a_post_tool_use_failure_is_marked_as_a_failure() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("u"), &unclaimed_email("demo")).await;
    let session = unique("sess");
    let at = Utc::now() - Duration::minutes(10);

    insert_event(
        &db.pool,
        &EventSpec::mcp_tool(&unique("evt"), &user, &session, "mcp__odoo__note_add")
            .failed()
            .at(at),
    )
    .await;

    let rows = list_mcp_tool_invocations(&db.pool, &DemoFilter::for_user(user))
        .await
        .expect("list mcp invocations");
    let row = rows.first().expect("one invocation");
    assert!(row.is_failure);
}
