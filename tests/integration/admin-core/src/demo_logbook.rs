//! `repositories::demo::logbook` — ordering, user scoping, and allow filtering.

use chrono::{Duration, Utc};
use systemprompt_web_admin::repositories::demo::filter::DemoFilter;
use systemprompt_web_admin::repositories::demo::logbook::{LogbookKind, list_demo_logbook};

use crate::fixtures::{
    DecisionSpec, EventSpec, insert_decision, insert_event, insert_user, unclaimed_email, unique,
};
use crate::tempdb::TempDb;

#[tokio::test]
async fn entries_are_newest_first_across_all_four_sources() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("u"), &unclaimed_email("demo")).await;
    let session = unique("sess");
    let at = Utc::now() - Duration::minutes(20);

    insert_event(
        &db.pool,
        &EventSpec::skill(&unique("evt"), &user, &session, "p:one").at(at),
    )
    .await;
    insert_event(
        &db.pool,
        &EventSpec::mcp_tool(&unique("evt"), &user, &session, "mcp__odoo__note_add")
            .at(at + Duration::minutes(1)),
    )
    .await;
    let mut held = DecisionSpec::allow(&unique("gd"), &user, &session);
    held.tool_name = "note_add";
    held.decision = "pending";
    held.policy = "require_approval";
    held.created_at = at + Duration::minutes(2);
    insert_decision(&db.pool, &held).await;

    let rows = list_demo_logbook(&db.pool, &DemoFilter::for_user(user), false)
        .await
        .expect("list logbook");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].kind, LogbookKind::Decision);
    assert_eq!(rows[1].kind, LogbookKind::McpTool);
    assert_eq!(rows[2].kind, LogbookKind::Skill);
    for pair in rows.windows(2) {
        assert!(pair[0].at >= pair[1].at);
    }
}

#[tokio::test]
async fn the_user_filter_excludes_every_other_users_entry() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("u"), &unclaimed_email("demo")).await;
    let other = insert_user(&db.pool, &unique("u"), &unclaimed_email("other")).await;
    let at = Utc::now() - Duration::minutes(20);

    let mine = unique("sess");
    let theirs = unique("sess");
    insert_event(
        &db.pool,
        &EventSpec::skill(&unique("evt"), &user, &mine, "p:mine").at(at),
    )
    .await;
    insert_event(
        &db.pool,
        &EventSpec::skill(&unique("evt"), &other, &theirs, "p:theirs").at(at),
    )
    .await;

    let rows = list_demo_logbook(&db.pool, &DemoFilter::for_user(user.clone()), false)
        .await
        .expect("list logbook");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].label, "p:mine");
    assert_eq!(rows[0].user_id, user);
}

#[tokio::test]
async fn include_allows_false_hides_the_per_request_authz_noise() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("u"), &unclaimed_email("demo")).await;
    let session = unique("sess");
    let at = Utc::now() - Duration::minutes(20);

    let mut noise = DecisionSpec::allow(&unique("gd"), &user, &session);
    noise.tool_name = "crm_lead_search";
    noise.policy = "authz_rule_based";
    noise.created_at = at;
    insert_decision(&db.pool, &noise).await;

    let mut signal = DecisionSpec::allow(&unique("gd"), &user, &session);
    signal.tool_name = "note_add";
    signal.decision = "deny";
    signal.policy = "secret_scan";
    signal.created_at = at + Duration::minutes(1);
    insert_decision(&db.pool, &signal).await;

    let mut approved = DecisionSpec::allow(&unique("gd"), &user, &session);
    approved.tool_name = "note_add";
    approved.policy = "require_approval";
    approved.created_at = at + Duration::minutes(2);
    insert_decision(&db.pool, &approved).await;

    let filtered = list_demo_logbook(&db.pool, &DemoFilter::for_user(user.clone()), false)
        .await
        .expect("list logbook");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].policy.as_deref(), Some("secret_scan"));

    let unfiltered = list_demo_logbook(&db.pool, &DemoFilter::for_user(user), true)
        .await
        .expect("list logbook with allows");
    assert_eq!(unfiltered.len(), 2);
}
