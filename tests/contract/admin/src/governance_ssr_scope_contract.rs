//! The governance console must show each enforcement plane through the
//! selected scope and preserve the filters an operator uses to triage it.

use axum::http::StatusCode;

use crate::app::{App, Call};
use crate::principal::Principal;
use crate::tempdb::TempDb;
use crate::{globals, principal, seed};

#[tokio::test(flavor = "multi_thread")]
async fn governance_tabs_filter_scoped_decisions_findings_and_hooks() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let credentials = principal::provision(&db.pool).await;
    let scoped_session = seed::unique("governance-scoped-session");
    let outside = seed::insert_user(
        &db.pool,
        &seed::unique("governance-outside"),
        "outside-governance@contract.test",
    )
    .await;
    let outside_session = seed::unique("governance-outside-session");
    let scoped_tool = "ScopedGovernanceTool";
    let scoped_allow_tool = "ScopedAllowGovernanceTool";
    let outside_tool = "OutsideGovernanceTool";
    seed::insert_session(&db.pool, &scoped_session, &credentials.non_admin_user_id).await;
    seed::insert_session(&db.pool, &outside_session, &outside).await;
    seed::insert_decision(
        &db.pool,
        &seed::DecisionSpec {
            id: seed::unique("scoped-deny"),
            user_id: &credentials.non_admin_user_id,
            session_id: &scoped_session,
            decision: "deny",
            policy: "scoped-policy",
            tool_name: scoped_tool,
        },
    )
    .await;
    seed::insert_decision(
        &db.pool,
        &seed::DecisionSpec {
            id: seed::unique("scoped-allow"),
            user_id: &credentials.non_admin_user_id,
            session_id: &scoped_session,
            decision: "allow",
            policy: "scoped-policy",
            tool_name: scoped_allow_tool,
        },
    )
    .await;
    seed::insert_decision(
        &db.pool,
        &seed::DecisionSpec {
            id: seed::unique("outside-deny"),
            user_id: &outside,
            session_id: &outside_session,
            decision: "deny",
            policy: "outside-policy",
            tool_name: outside_tool,
        },
    )
    .await;
    let request_id = seed::unique("governance-finding-request");
    seed::insert_request(
        &db.pool,
        &seed::RequestSpec {
            id: request_id.clone(),
            user_id: &credentials.non_admin_user_id,
            session_id: Some(&scoped_session),
            trace_id: None,
            context_id: None,
            status: "completed",
        },
    )
    .await;
    sqlx::query("INSERT INTO ai_safety_findings (id, ai_request_id, phase, severity, category, scanner, excerpt, blocked) VALUES ($1, $2, 'request', 'high', 'scoped-category', 'contract-scanner', 'scoped finding excerpt', true)")
        .bind(seed::unique("scoped-finding")).bind(&request_id).execute(&*db.pool).await.expect("insert scoped safety finding");
    sqlx::query("INSERT INTO ai_safety_findings (id, ai_request_id, phase, severity, category, scanner, excerpt, blocked) VALUES ($1, $2, 'response', 'low', 'unblocked-category', 'contract-scanner', 'unblocked finding excerpt', false)")
        .bind(seed::unique("unblocked-finding")).bind(&request_id).execute(&*db.pool).await.expect("insert unblocked safety finding");
    let outside_request = seed::unique("outside-finding-request");
    seed::insert_request(
        &db.pool,
        &seed::RequestSpec {
            id: outside_request.clone(),
            user_id: &outside,
            session_id: Some(&outside_session),
            trace_id: None,
            context_id: None,
            status: "completed",
        },
    )
    .await;
    sqlx::query("INSERT INTO ai_safety_findings (id, ai_request_id, phase, severity, category, scanner, excerpt, blocked) VALUES ($1, $2, 'request', 'high', 'outside-category', 'contract-scanner', 'outside finding excerpt', true)")
        .bind(seed::unique("outside-finding")).bind(&outside_request).execute(&*db.pool).await.expect("insert outside safety finding");
    seed::insert_event(
        &db.pool,
        &credentials.non_admin_user_id,
        &scoped_session,
        scoped_tool,
    )
    .await;
    let app = App::new(&db.pool, credentials);

    let (status, body) = app
        .call(Call::get(
            "/admin/governance?group=contract-group&attention=1",
            Principal::Admin,
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "decision tab: {body}");
    assert!(
        body.contains(scoped_tool),
        "the scoped deny must survive attention filtering: {body}"
    );
    assert!(
        !body.contains(scoped_allow_tool),
        "attention hides a scoped call with only an allow"
    );
    assert!(
        !body.contains(outside_tool),
        "group scope must not leak another user's decision"
    );
    assert!(
        body.contains("scoped-policy"),
        "the decision page retains the matching policy: {body}"
    );

    let (status, body) = app
        .call(Call::get(
            "/admin/governance?tab=safety&group=contract-group&blocked=blocked",
            Principal::Admin,
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "safety tab: {body}");
    let table_start = body
        .find("<caption class=\"sp-u-sr-only\">Gateway safety findings</caption>")
        .expect("safety findings table");
    let table_tail = &body[table_start..];
    let table = &table_tail[..table_tail.find("</table>").expect("safety table closes")];
    assert!(
        table.contains("scoped-category") && table.contains("scoped finding excerpt"),
        "the blocked scoped finding is rendered in the results table: {table}"
    );
    assert!(
        !table.contains("unblocked-category") && !table.contains("outside-category"),
        "blocked filter and group scope exclude their respective rows: {table}"
    );
    assert!(
        table.contains("blocked"),
        "the blocked filter retains its enforcement outcome in the result row: {table}"
    );

    let (status, body) = app
        .call(Call::get(
            "/admin/governance?tab=hooks&group=contract-group",
            Principal::Admin,
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "hooks tab: {body}");
    assert!(body.contains(scoped_tool), "post-tool hook is rendered");
    assert!(
        body.contains("PreToolUse"),
        "policy hook remains visible beside post-tool events"
    );
    db.cleanup().await;
}
