//! `repositories::projects::members` — attribution membership, and the same
//! two-writer rule groups carry.

use systemprompt_web_admin::repositories::projects::activity::list_project_sessions;
use systemprompt_web_admin::repositories::projects::members::{
    delete_project_member, insert_project_member, list_project_ids_for_user, list_project_members,
    replace_directory_project_memberships,
};
use systemprompt_web_admin::repositories::scope::{Attribution, ScopeQuery, ScopeTarget};

use crate::fixtures::{
    RequestSpec, insert_request, insert_session, insert_user, unclaimed_email, unique,
};
use crate::tempdb::TempDb;
use systemprompt_web_shared::ProjectId;

#[tokio::test]
async fn a_manual_member_is_listed_with_its_source() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("member")).await;
    let admin = insert_user(&db.pool, &unique("admin"), &unclaimed_email("admin")).await;

    insert_project_member(&db.pool, &ProjectId::new("core"), &user, &admin, None)
        .await
        .expect("add");

    let members = list_project_members(&db.pool, &ProjectId::new("core"))
        .await
        .expect("list");
    let row = members
        .iter()
        .find(|m| m.user_id == user.as_str())
        .expect("the member is listed");
    assert_eq!(row.sources, vec!["manual".to_owned()]);
    assert_eq!(
        list_project_ids_for_user(&db.pool, &user)
            .await
            .expect("read"),
        vec![ProjectId::new("core")]
    );
    db.cleanup().await;
}

// Why: removing it here would be undone at the member's next sign-in, so the
// refusal says what is actually true — the change belongs in AD.
#[tokio::test]
async fn a_directory_sourced_member_cannot_be_removed_by_hand() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("sso")).await;
    sqlx::query(
        "INSERT INTO project_ad_mappings (ad_group, project_id, source) \
         VALUES ('AD-Core', 'core', 'yaml') ON CONFLICT DO NOTHING",
    )
    .execute(&*db.pool)
    .await
    .expect("map");
    replace_directory_project_memberships(&db.pool, &user, &["AD-Core".to_owned()])
        .await
        .expect("sign-in");

    let err = delete_project_member(&db.pool, &ProjectId::new("core"), &user)
        .await
        .expect_err("refused");

    assert_eq!(err.status().as_u16(), 409);
    assert_eq!(
        list_project_ids_for_user(&db.pool, &user)
            .await
            .expect("read"),
        vec![ProjectId::new("core")],
        "the membership survives the refusal"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn removing_a_member_who_is_not_one_is_a_not_found() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("stranger")).await;

    let err = delete_project_member(&db.pool, &ProjectId::new("core"), &user)
        .await
        .expect_err("refused");

    assert_eq!(err.status().as_u16(), 404);
    db.cleanup().await;
}

// A directory assertion is a replacement of directory evidence, not of every
// way an operator may have assigned attribution.  Losing the AD claim must
// remove the former project while preserving the manual one.
#[tokio::test]
async fn directory_refresh_replaces_project_claims_but_preserves_manual_attribution() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("mixed-sources")).await;
    let admin = insert_user(&db.pool, &unique("admin"), &unclaimed_email("grantor")).await;
    let manual = ProjectId::new("core");
    let directory = ProjectId::new("commerce");

    insert_project_member(&db.pool, &manual, &user, &admin, None)
        .await
        .expect("manual attribution");
    sqlx::query(
        "INSERT INTO project_ad_mappings (ad_group, project_id, source) \
         VALUES ('AD-Commerce', 'commerce', 'yaml') ON CONFLICT DO NOTHING",
    )
    .execute(&*db.pool)
    .await
    .expect("map directory group");

    replace_directory_project_memberships(&db.pool, &user, &["AD-Commerce".to_owned()])
        .await
        .expect("first assertion");
    let mut first = list_project_ids_for_user(&db.pool, &user)
        .await
        .expect("first projects");
    first.sort();
    assert_eq!(first, vec![directory.clone(), manual.clone()]);

    replace_directory_project_memberships(&db.pool, &user, &[])
        .await
        .expect("assertion after leaving directory group");
    assert_eq!(
        list_project_ids_for_user(&db.pool, &user)
            .await
            .expect("remaining projects"),
        vec![manual],
        "refresh deletes only adfs rows; dashboard attribution survives"
    );
    db.cleanup().await;
}

// A previously revoked manual grant is audit history, not a permanent bar on
// granting the same person access again.  The upsert must revive that row and
// restore its visibility to project reads.
#[tokio::test]
async fn regranting_a_revoked_manual_project_membership_revives_it() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("regrant")).await;
    let admin = insert_user(&db.pool, &unique("admin"), &unclaimed_email("regrantor")).await;
    let project = ProjectId::new("core");

    insert_project_member(&db.pool, &project, &user, &admin, None)
        .await
        .expect("initial grant");
    sqlx::query(
        "UPDATE project_members SET revoked_at = CURRENT_TIMESTAMP
         WHERE project_id = $1 AND user_id = $2 AND source = 'manual'",
    )
    .bind(project.as_str())
    .bind(user.as_str())
    .execute(&*db.pool)
    .await
    .expect("revoke old grant");

    insert_project_member(&db.pool, &project, &user, &admin, None)
        .await
        .expect("regrant revives old row");
    assert_eq!(
        list_project_ids_for_user(&db.pool, &user)
            .await
            .expect("project is readable again"),
        vec![project]
    );
    let revoked_at: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT revoked_at FROM project_members
         WHERE project_id = $1 AND user_id = $2 AND source = 'manual'",
    )
    .bind("core")
    .bind(user.as_str())
    .fetch_one(&*db.pool)
    .await
    .expect("manual row");
    assert!(
        revoked_at.is_none(),
        "the old grant was reactivated in place"
    );
    db.cleanup().await;
}

// Project detail must draw sessions through membership, and retain sessions
// with no requests so an operator can distinguish an idle connection from a
// missing session.  A non-member's busy session must never leak into it.
#[tokio::test]
async fn project_activity_scopes_sessions_and_keeps_idle_sessions() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let member = insert_user(
        &db.pool,
        &unique("member"),
        &unclaimed_email("session-member"),
    )
    .await;
    let outsider = insert_user(
        &db.pool,
        &unique("outsider"),
        &unclaimed_email("session-outsider"),
    )
    .await;
    let admin = insert_user(
        &db.pool,
        &unique("admin"),
        &unclaimed_email("session-admin"),
    )
    .await;
    let project = ProjectId::new("core");
    insert_project_member(&db.pool, &project, &member, &admin, None)
        .await
        .expect("project member");
    let busy = unique("busy-session");
    let idle = unique("idle-session");
    let other = unique("other-session");
    insert_session(&db.pool, &busy, &member).await;
    insert_session(&db.pool, &idle, &member).await;
    insert_session(&db.pool, &other, &outsider).await;
    let mut request = RequestSpec::completed(&unique("request"), &member);
    request.session_id = Some(&busy);
    request.cost_microdollars = 91;
    insert_request(&db.pool, &request).await;
    let mut outsider_request = RequestSpec::completed(&unique("request"), &outsider);
    outsider_request.session_id = Some(&other);
    insert_request(&db.pool, &outsider_request).await;

    let query = ScopeQuery::new(ScopeTarget::Project(&project), Attribution::Member, 1);
    let rows = list_project_sessions(&db.pool, &query, 10)
        .await
        .expect("project sessions");
    let busy_row = rows
        .iter()
        .find(|row| row.session_id.as_str() == busy)
        .expect("busy row");
    assert_eq!(busy_row.requests, 1);
    assert_eq!(busy_row.cost_microdollars, 91);
    assert!(
        rows.iter()
            .any(|row| row.session_id.as_str() == idle && row.requests == 0)
    );
    assert!(!rows.iter().any(|row| row.session_id.as_str() == other));
    db.cleanup().await;
}
