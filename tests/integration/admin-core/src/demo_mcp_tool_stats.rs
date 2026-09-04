//! `repositories::demo::mcp_tools::list_mcp_tool_stats` — the bare-name join.
//!
//! Hook events name a tool `mcp__<server>__<tool>` while `governance_decisions`
//! and `approval_requests` carry the bare name, so the rollup only lines up if
//! the join normalises.

use chrono::{Duration, Utc};
use systemprompt_web_admin::repositories::demo::filter::DemoFilter;
use systemprompt_web_admin::repositories::demo::mcp_tools::list_mcp_tool_stats;

use crate::fixtures::{
    ApprovalSpec, DecisionSpec, EventSpec, insert_approval, insert_decision, insert_event,
    insert_user, unclaimed_email, unique,
};
use crate::tempdb::TempDb;

#[tokio::test]
async fn decisions_and_approvals_join_on_the_bare_tool_name() {
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
            "mcp__odoo__crm_lead_delete",
        )
        .at(at),
    )
    .await;
    insert_event(
        &db.pool,
        &EventSpec::mcp_tool(
            &unique("evt"),
            &user,
            &session,
            "mcp__odoo__crm_lead_delete",
        )
        .failed()
        .at(at + Duration::seconds(5)),
    )
    .await;

    let mut denied = DecisionSpec::allow(&unique("gd"), &user, &session);
    denied.tool_name = "crm_lead_delete";
    denied.decision = "deny";
    denied.policy = "tool_blocklist";
    insert_decision(&db.pool, &denied).await;

    insert_approval(
        &db.pool,
        &ApprovalSpec {
            call_id: unique("call"),
            requested_by: &user,
            session_id: None,
            server_name: "odoo",
            tool_name: "crm_lead_delete",
            status: "pending",
        },
    )
    .await;

    let stats = list_mcp_tool_stats(&db.pool, &DemoFilter::for_user(user))
        .await
        .expect("list stats");
    let row = stats
        .iter()
        .find(|s| s.tool == "crm_lead_delete")
        .expect("crm_lead_delete stats");
    assert_eq!(row.server, "odoo");
    assert_eq!(row.call_count, 2);
    assert_eq!(row.failure_count, 1);
    assert!((row.failure_rate - 0.5).abs() < f64::EPSILON);
    assert_eq!(row.distinct_users, 1);
    assert_eq!(row.denied, 1);
    assert_eq!(row.approval_pending, 1);
}
