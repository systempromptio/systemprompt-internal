//! `repositories::demo::skill_invocations` — the attribution window.

use chrono::{Duration, Utc};
use systemprompt_web_admin::repositories::demo::filter::DemoFilter;
use systemprompt_web_admin::repositories::demo::skill_invocations::{
    list_skill_invocations, list_skill_totals,
};

use crate::fixtures::{
    EventSpec, RequestSpec, insert_event, insert_request, insert_skill_event, insert_user,
    unclaimed_email, unique,
};
use crate::tempdb::TempDb;

#[tokio::test]
async fn a_request_inside_the_window_is_attributed_to_the_invocation() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("u"), &unclaimed_email("demo")).await;
    let session = unique("sess");
    let start = Utc::now() - Duration::minutes(30);

    insert_skill_event(
        &db.pool,
        &EventSpec::skill(&unique("evt"), &user, &session, "p:alpha").at(start),
    )
    .await;
    insert_event(
        &db.pool,
        &EventSpec::tool_use(&unique("evt"), &user, &session).at(start + Duration::minutes(1)),
    )
    .await;

    let mut req = RequestSpec::completed(&unique("req"), &user);
    req.created_at = start + Duration::seconds(30);
    insert_request(&db.pool, &req).await;

    let rows = list_skill_invocations(&db.pool, &DemoFilter::all_users())
        .await
        .expect("list invocations");
    let alpha = rows
        .iter()
        .find(|r| r.skill == "p:alpha")
        .expect("alpha invocation");
    assert_eq!(alpha.request_count, 1);
    assert_eq!(alpha.total_tokens, 120);
    assert_eq!(alpha.cost_microdollars, 5_000);
}

#[tokio::test]
async fn a_request_after_the_next_skill_belongs_to_the_second_invocation() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("u"), &unclaimed_email("demo")).await;
    let session = unique("sess");
    let start = Utc::now() - Duration::minutes(30);

    insert_skill_event(
        &db.pool,
        &EventSpec::skill(&unique("evt"), &user, &session, "p:first").at(start),
    )
    .await;
    insert_skill_event(
        &db.pool,
        &EventSpec::skill(&unique("evt"), &user, &session, "p:second")
            .at(start + Duration::minutes(2)),
    )
    .await;

    let mut req = RequestSpec::completed(&unique("req"), &user);
    req.created_at = start + Duration::minutes(3);
    insert_request(&db.pool, &req).await;

    let rows = list_skill_invocations(&db.pool, &DemoFilter::all_users())
        .await
        .expect("list invocations");
    let first = rows.iter().find(|r| r.skill == "p:first").expect("first");
    let second = rows.iter().find(|r| r.skill == "p:second").expect("second");
    assert_eq!(first.request_count, 0);
    assert_eq!(second.request_count, 1);
}

#[tokio::test]
async fn a_request_past_the_pad_after_the_last_event_is_not_attributed() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("u"), &unclaimed_email("demo")).await;
    let session = unique("sess");
    let start = Utc::now() - Duration::minutes(30);

    insert_skill_event(
        &db.pool,
        &EventSpec::skill(&unique("evt"), &user, &session, "p:late").at(start),
    )
    .await;

    let mut req = RequestSpec::completed(&unique("req"), &user);
    req.created_at = start + Duration::minutes(6);
    insert_request(&db.pool, &req).await;

    let rows = list_skill_invocations(&db.pool, &DemoFilter::all_users())
        .await
        .expect("list invocations");
    let late = rows.iter().find(|r| r.skill == "p:late").expect("late");
    assert_eq!(late.request_count, 0);
}

#[tokio::test]
async fn another_users_request_is_never_attributed() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("u"), &unclaimed_email("demo")).await;
    let other = insert_user(&db.pool, &unique("u"), &unclaimed_email("other")).await;
    let session = unique("sess");
    let start = Utc::now() - Duration::minutes(30);

    insert_skill_event(
        &db.pool,
        &EventSpec::skill(&unique("evt"), &user, &session, "p:mine").at(start),
    )
    .await;

    let mut req = RequestSpec::completed(&unique("req"), &other);
    req.created_at = start + Duration::seconds(30);
    insert_request(&db.pool, &req).await;

    let rows = list_skill_invocations(&db.pool, &DemoFilter::all_users())
        .await
        .expect("list invocations");
    let mine = rows.iter().find(|r| r.skill == "p:mine").expect("mine");
    assert_eq!(mine.request_count, 0);
}

#[tokio::test]
async fn totals_sum_the_invocations_of_each_skill() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("u"), &unclaimed_email("demo")).await;
    let other = insert_user(&db.pool, &unique("u"), &unclaimed_email("other")).await;
    let start = Utc::now() - Duration::minutes(30);

    let s1 = unique("sess");
    let s2 = unique("sess");
    insert_skill_event(
        &db.pool,
        &EventSpec::skill(&unique("evt"), &user, &s1, "p:shared").at(start),
    )
    .await;
    insert_skill_event(
        &db.pool,
        &EventSpec::skill(&unique("evt"), &other, &s2, "p:shared").at(start + Duration::minutes(1)),
    )
    .await;

    let totals = list_skill_totals(&db.pool, &DemoFilter::all_users())
        .await
        .expect("list totals");
    let shared = totals
        .iter()
        .find(|t| t.skill == "p:shared")
        .expect("shared total");
    assert_eq!(shared.invocation_count, 2);
    assert_eq!(shared.distinct_users, 2);

    let scoped = list_skill_totals(&db.pool, &DemoFilter::for_user(user.clone()))
        .await
        .expect("list totals for user");
    let shared = scoped
        .iter()
        .find(|t| t.skill == "p:shared")
        .expect("shared total for user");
    assert_eq!(shared.invocation_count, 1);
    assert_eq!(shared.distinct_users, 1);
}
