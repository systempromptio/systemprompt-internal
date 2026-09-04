//! `repositories::demo::mcp_tools::list_mcp_tool_stats` — the bare-name join.
//!
//! Hook events name a tool `mcp__<server>__<tool>`. `approval_requests` carries
//! the bare name and `governance_decisions` carries either, so the rollup only
//! lines up if the join normalises both sides.

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

    // The govern hook writes the wire name; the MCP proxy writes the bare name.
    // Both must land on the same row.
    let mut denied = DecisionSpec::allow(&unique("gd"), &user, &session);
    denied.tool_name = "mcp__odoo__crm_lead_delete";
    denied.decision = "deny";
    denied.policy = "tool_blocklist";
    denied.plugin_id = Some("systemprompt-business");
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

// A real per-tool verdict and the per-request server authorization share the
// `authz_rule_based` policy. Only the pair (policy, plugin_id) separates them,
// so excluding by policy name alone zeroes every allowed count.
#[tokio::test]
async fn a_real_allow_verdict_counts_but_the_server_authorization_does_not() {
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

    let mut verdict = DecisionSpec::allow(&unique("gd"), &user, &session);
    verdict.tool_name = "crm_lead_search";
    verdict.policy = "authz_rule_based";
    verdict.plugin_id = Some("systemprompt-business");
    insert_decision(&db.pool, &verdict).await;

    // The server authorization: same policy, no plugin_id, and the tool_name is
    // the server rather than a tool.
    let mut server_auth = DecisionSpec::allow(&unique("gd"), &user, &session);
    server_auth.tool_name = "odoo";
    server_auth.policy = "authz_rule_based";
    insert_decision(&db.pool, &server_auth).await;

    let mut legacy_authz = DecisionSpec::allow(&unique("gd"), &user, &session);
    legacy_authz.tool_name = "crm_lead_search";
    legacy_authz.policy = "authz";
    insert_decision(&db.pool, &legacy_authz).await;

    let stats = list_mcp_tool_stats(&db.pool, &DemoFilter::for_user(user))
        .await
        .expect("list stats");
    let row = stats
        .iter()
        .find(|s| s.tool == "crm_lead_search")
        .expect("crm_lead_search stats");
    assert_eq!(
        row.allowed, 1,
        "only the plugin-attributed verdict counts as an allowed call"
    );
    assert!(
        !stats.iter().any(|s| s.tool == "odoo"),
        "the server authorization row must not become a tool"
    );
}
