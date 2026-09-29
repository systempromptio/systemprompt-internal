//! `ai_request_scopes` — the scope stamped on a request when it lands.
//!
//! The trigger on core's `ai_requests` copies the person's primary group and
//! project into a row of its own, so exclusive attribution reads what was
//! true at the time rather than the membership of the moment the page is
//! drawn. Moving a person changes where their next request goes and nothing
//! else.

use sqlx::PgPool;
use systemprompt_web_admin::repositories::people_usage::get_scope_usage;
use systemprompt_web_admin::repositories::people_usage::totals::list_scope_totals;
use systemprompt_web_admin::repositories::scope::defaults::set_scope_defaults;
use systemprompt_web_admin::repositories::scope::membership::UNATTRIBUTED;
use systemprompt_web_admin::repositories::scope::{
    Attribution, ScopeKind, ScopeQuery, ScopeTarget,
};
use systemprompt_web_shared::GroupId;

use crate::fixtures::{
    RequestSpec, insert_group, insert_project, insert_request, insert_user, unclaimed_email,
    unique, unique_group, unique_project,
};
use crate::tempdb::TempDb;

const WINDOW: i32 = 30;

const fn exclusive(id: &GroupId) -> ScopeQuery<'_> {
    ScopeQuery::new(ScopeTarget::Group(id), Attribution::Exclusive, WINDOW)
}

#[derive(Debug, PartialEq, Eq)]
struct StampedScope {
    user_id: String,
    group_id: Option<String>,
    project_id: Option<String>,
    source: String,
}

async fn find_stamp(pool: &PgPool, request_id: &str) -> Option<StampedScope> {
    sqlx::query_as::<_, (String, Option<String>, Option<String>, String)>(
        "SELECT user_id, group_id, project_id, source
         FROM ai_request_scopes WHERE request_id = $1",
    )
    .bind(request_id)
    .fetch_optional(pool)
    .await
    .expect("read stamp")
    .map(|(user_id, group_id, project_id, source)| StampedScope {
        user_id,
        group_id,
        project_id,
        source,
    })
}

#[tokio::test]
async fn a_request_is_stamped_with_the_primary_group_and_project_when_it_lands() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let group = unique_group("grp");
    let project = unique_project("prj");
    insert_group(&db.pool, &group, group.as_str()).await;
    insert_project(&db.pool, &project, project.as_str()).await;
    let person = insert_user(&db.pool, &unique("user"), &unclaimed_email("stamped")).await;
    set_scope_defaults(&db.pool, &person, Some(&group), Some(&project))
        .await
        .expect("set defaults");

    let request = unique("req");
    insert_request(&db.pool, &RequestSpec::completed(&request, &person)).await;

    assert_eq!(
        find_stamp(&db.pool, &request).await,
        Some(StampedScope {
            user_id: person.as_str().to_owned(),
            group_id: Some(group.as_str().to_owned()),
            project_id: Some(project.as_str().to_owned()),
            source: "primary".to_owned(),
        }),
        "the trigger copies both primaries the moment the request is inserted"
    );

    db.cleanup().await;
}

// Why: the whole point of stamping. Under the old membership join, moving a
// person rewrote every request they had ever made.
#[tokio::test]
async fn moving_the_primary_leaves_earlier_requests_where_they_were() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let before = unique_group("grp");
    let after = unique_group("grp");
    insert_group(&db.pool, &before, before.as_str()).await;
    insert_group(&db.pool, &after, after.as_str()).await;
    let person = insert_user(&db.pool, &unique("user"), &unclaimed_email("moved")).await;

    set_scope_defaults(&db.pool, &person, Some(&before), None)
        .await
        .expect("first primary");
    let early = unique("req");
    insert_request(&db.pool, &RequestSpec::completed(&early, &person)).await;

    set_scope_defaults(&db.pool, &person, Some(&after), None)
        .await
        .expect("second primary");
    let late = unique("req");
    insert_request(&db.pool, &RequestSpec::completed(&late, &person)).await;

    let early_stamp = find_stamp(&db.pool, &early).await.expect("early stamp");
    let late_stamp = find_stamp(&db.pool, &late).await.expect("late stamp");
    assert_eq!(early_stamp.group_id.as_deref(), Some(before.as_str()));
    assert_eq!(late_stamp.group_id.as_deref(), Some(after.as_str()));

    let kept = get_scope_usage(&db.pool, &exclusive(&before))
        .await
        .expect("usage before");
    let gained = get_scope_usage(&db.pool, &exclusive(&after))
        .await
        .expect("usage after");
    assert_eq!(
        kept.requests, 1,
        "the group the person left keeps the request made while they were in it"
    );
    assert_eq!(
        gained.requests, 1,
        "the group they joined gets only what came after"
    );

    db.cleanup().await;
}

// Why: the schema install seeds traffic of its own, so the remainder is read
// as a delta rather than asserted to be exactly this test's request.
async fn unattributed_requests(pool: &PgPool) -> i64 {
    list_scope_totals(pool, ScopeKind::Group, Attribution::Exclusive, WINDOW)
        .await
        .expect("totals")
        .iter()
        .find(|row| row.scope_id == UNATTRIBUTED)
        .map_or(0, |row| row.requests)
}

#[tokio::test]
async fn a_request_with_no_primary_is_stamped_unattributed_and_stays_so() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let group = unique_group("grp");
    insert_group(&db.pool, &group, group.as_str()).await;
    let person = insert_user(&db.pool, &unique("user"), &unclaimed_email("unkeyed")).await;

    let remainder_before = unattributed_requests(&db.pool).await;

    let request = unique("req");
    insert_request(&db.pool, &RequestSpec::completed(&request, &person)).await;
    let stamp = find_stamp(&db.pool, &request)
        .await
        .expect("a row is written regardless");
    assert_eq!(stamp.group_id, None);
    assert_eq!(stamp.project_id, None);

    // Keying the person afterwards does not reach back.
    set_scope_defaults(&db.pool, &person, Some(&group), None)
        .await
        .expect("set defaults");
    assert_eq!(
        unattributed_requests(&db.pool).await,
        remainder_before + 1,
        "the request made before the person had a primary stays in the remainder"
    );
    let totals = list_scope_totals(&db.pool, ScopeKind::Group, Attribution::Exclusive, WINDOW)
        .await
        .expect("totals");
    assert!(
        !totals.iter().any(|row| row.scope_id == group.as_str()),
        "the group gained nothing from a key assigned after the fact"
    );

    db.cleanup().await;
}
