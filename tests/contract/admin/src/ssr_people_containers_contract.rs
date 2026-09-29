//! Populated group and project detail pages: the status matrix drives their
//! empty shells, but these pages only exercise their member rows when a real
//! membership is present.
//!
//! This instance seeds no project, so the `core` project every case reads is
//! created here beside the group.

use axum::http::StatusCode;
use systemprompt::identifiers::MarketplaceId;
use systemprompt_web_admin::repositories::bridge::issue_api_key;
use systemprompt_web_admin::repositories::groups::marketplaces::set_group_marketplaces;

use crate::app::{App, Call};
use crate::principal::Principal;
use crate::tempdb::TempDb;
use crate::{globals, principal, seed};

async fn seed_member_with_group_and_project(
    pool: &sqlx::PgPool,
) -> (String, String, systemprompt::identifiers::UserId, String) {
    let id = seed::unique("detail-member");
    let email = format!("{id}@contract.test");
    let user = seed::insert_user(pool, &id, &email).await;
    let group = seed::unique("detail-group");
    sqlx::query(
        "INSERT INTO projects (id, name, source) VALUES ('core', 'Core', 'dashboard')
         ON CONFLICT (id) DO NOTHING",
    )
    .execute(pool)
    .await
    .expect("create the core project");
    sqlx::query(
        "INSERT INTO groups (id, name, source) VALUES ($1, 'SSR detail group', 'dashboard')",
    )
    .bind(&group)
    .execute(pool)
    .await
    .expect("create group");
    sqlx::query(
        "INSERT INTO group_members (group_id, user_id, source, granted_by)
         VALUES ($1, $2, 'manual', $2)",
    )
    .bind(&group)
    .bind(user.as_str())
    .execute(pool)
    .await
    .expect("add group member");
    sqlx::query(
        "INSERT INTO project_members (project_id, user_id, source, granted_by)
         VALUES ('core', $1, 'manual', $1)",
    )
    .bind(user.as_str())
    .execute(pool)
    .await
    .expect("add project member");
    (id, email, user, group)
}

#[tokio::test(flavor = "multi_thread")]
async fn group_and_project_member_tabs_render_the_seeded_person() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let credentials = principal::provision(&db.pool).await;
    let app = App::new(&db.pool, credentials);
    let (_, email, _, group) = seed_member_with_group_and_project(&db.pool).await;

    let group_path = format!("/admin/groups/{group}?tab=members");
    let (status, body) = app.call(Call::get(&group_path, Principal::Admin)).await;
    assert_eq!(status, StatusCode::OK, "group detail: {body}");
    assert!(body.contains("SSR detail group"));
    assert!(body.contains(&email), "member row is rendered: {body}");
    assert!(body.contains(&group), "member actions retain the group id");

    let (status, body) = app
        .call(Call::get(
            "/admin/projects/core?tab=members",
            Principal::Admin,
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "project detail: {body}");
    assert!(body.contains("Core"));
    assert!(body.contains(&email), "member row is rendered: {body}");
    assert!(body.contains("data-project-id=\"core\""));
    db.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn project_usage_and_print_report_render_seeded_request_and_session() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let credentials = principal::provision(&db.pool).await;
    let app = App::new(&db.pool, credentials);
    let (_, email, user, _) = seed_member_with_group_and_project(&db.pool).await;
    sqlx::query(
        "INSERT INTO user_scope_defaults (user_id, primary_project_id, source)
         VALUES ($1, 'core', 'manual')",
    )
    .bind(user.as_str())
    .execute(&*db.pool)
    .await
    .expect("stamp core as the primary project");
    let session = seed::unique("project-session");
    seed::insert_session(&db.pool, &session, &user).await;
    seed::insert_request(
        &db.pool,
        &seed::RequestSpec {
            id: seed::unique("project-request"),
            user_id: &user,
            session_id: Some(&session),
            trace_id: None,
            context_id: None,
            status: "completed",
        },
    )
    .await;

    let (status, body) = app
        .call(Call::get(
            "/admin/projects/core?tab=usage",
            Principal::Admin,
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "usage tab: {body}");
    assert!(
        body.contains("claude-contract-model"),
        "model row is populated"
    );
    assert!(body.contains(&session), "session row is populated");

    let (status, body) = app
        .call(Call::get(
            "/admin/projects/core/report?print=1",
            Principal::Admin,
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "project report: {body}");
    assert!(body.contains("data-project-report=\"core\""));
    assert!(body.contains("data-print-on-load"));
    assert!(
        body.contains(&email),
        "report renders its populated member row"
    );
    assert!(body.contains("claude-contract-model"));
    assert!(body.contains(&session));
    db.cleanup().await;
}

// `unassigned` uses a distinct page model: it has no stored membership, yet
// its derived members must be rendered with destinations an admin can use to
// place them.
#[tokio::test(flavor = "multi_thread")]
async fn unassigned_detail_renders_a_derived_member_and_assignment_targets() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let credentials = principal::provision(&db.pool).await;
    let app = App::new(&db.pool, credentials);
    let id = seed::unique("unassigned-member");
    let email = format!("{id}@contract.test");
    seed::insert_user(&db.pool, &id, &email).await;
    let target = seed::unique("assignment-target");
    sqlx::query(
        "INSERT INTO groups (id, name, source) VALUES ($1, 'Assignment target', 'dashboard')",
    )
    .bind(&target)
    .execute(&*db.pool)
    .await
    .expect("create target group");

    let (status, body) = app
        .call(Call::get(
            "/admin/groups/unassigned?tab=members",
            Principal::Admin,
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "unassigned detail: {body}");
    assert!(body.contains(&email), "derived member is shown: {body}");
    assert!(
        body.contains(&target),
        "admin can select a real destination"
    );
    db.cleanup().await;
}

// The account page is where an operator checks both memberships and the
// primary attribution choices.  The page must show the held containers and
// retain a manual primary, rather than presenting every configured container
// as if it belonged to this person.
#[tokio::test(flavor = "multi_thread")]
async fn user_membership_tab_renders_held_containers_and_manual_primaries() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let credentials = principal::provision(&db.pool).await;
    let app = App::new(&db.pool, credentials);
    let (_, email, user, group) = seed_member_with_group_and_project(&db.pool).await;
    sqlx::query(
        "INSERT INTO user_scope_defaults (user_id, primary_group_id, primary_project_id, source)
         VALUES ($1, $2, 'core', 'manual')",
    )
    .bind(user.as_str())
    .bind(&group)
    .execute(&*db.pool)
    .await
    .expect("set manual primaries");

    let path = format!("/admin/users/{}?tab=membership", user.as_str());
    let (status, body) = app.call(Call::get(&path, Principal::Admin)).await;
    assert_eq!(status, StatusCode::OK, "membership tab: {body}");
    assert!(body.contains(&email));
    assert!(body.contains("SSR detail group"));
    assert!(body.contains("Core"));
    assert!(
        body.contains(&format!("data-membership-id=\"{group}\"")),
        "the held group is rendered as a membership control"
    );
    assert!(
        body.contains("data-membership-id=\"core\""),
        "the held project is rendered as a membership control"
    );
    assert!(body.contains("Decided by"));
    assert!(body.contains("manual"));
    db.cleanup().await;
}

// Credential listings are an audit surface: the operator needs the label and
// revocation control, while the one-time token secret must never be rendered
// again after issuance.
#[tokio::test(flavor = "multi_thread")]
async fn user_devices_tab_shows_a_pat_prefix_but_never_the_pat_secret() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let credentials = principal::provision(&db.pool).await;
    let app = App::new(&db.pool, credentials);
    let id = seed::unique("pat-owner");
    let email = format!("{id}@contract.test");
    let user = seed::insert_user(&db.pool, &id, &email).await;
    let issued = issue_api_key(&db.pool, &user, "SSR audit token", None)
        .await
        .expect("issue PAT");

    let path = format!("/admin/users/{}?tab=devices", user.as_str());
    let (status, body) = app.call(Call::get(&path, Principal::Admin)).await;
    assert_eq!(status, StatusCode::OK, "devices tab: {body}");
    assert!(body.contains("SSR audit token"));
    assert!(body.contains(&issued.key_prefix));
    assert!(
        body.contains(&issued.id),
        "the row carries its revoke target"
    );
    assert!(body.contains("data-kind=\"pat\""));
    assert!(
        !body.contains(&issued.secret),
        "the PAT secret is available only at issue time, never on the account page"
    );
    db.cleanup().await;
}

// The Access tab is an explanation of the enforcement decision.  A shared
// group rule must resolve as an allowed workspace with its group provenance;
// it must not masquerade as a personal override the operator could remove.
#[tokio::test(flavor = "multi_thread")]
async fn user_access_tab_explains_a_group_grant_without_offering_a_personal_override() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let credentials = principal::provision(&db.pool).await;
    let app = App::new(&db.pool, credentials);
    let (_, _, user, group) = seed_member_with_group_and_project(&db.pool).await;
    let entity = "enterprise-demo";
    sqlx::query(
        "INSERT INTO access_control_entities (entity_type, entity_id, default_included, source)
         VALUES ('marketplace', $1, false, 'dashboard')
         ON CONFLICT (entity_type, entity_id) DO UPDATE SET default_included = false",
    )
    .bind(entity)
    .execute(&*db.pool)
    .await
    .expect("register marketplace access entity");
    let shared_rule_id = seed::unique("group-rule");
    sqlx::query(
        "INSERT INTO access_control_rules
             (id, entity_type, entity_id, rule_type, rule_value, access, justification, source)
         VALUES ($1, 'marketplace', $2, 'group', $3, 'allow',
                 'the shared delivery team workspace', 'dashboard')",
    )
    .bind(&shared_rule_id)
    .bind(entity)
    .bind(&group)
    .execute(&*db.pool)
    .await
    .expect("grant workspace through group");

    let path = format!("/admin/users/{}?tab=access", user.as_str());
    let (status, body) = app.call(Call::get(&path, Principal::Admin)).await;
    assert_eq!(status, StatusCode::OK, "access tab: {body}");
    let entity_marker = format!("data-entity-id=\"{entity}\"");
    let row_start = body
        .find(&entity_marker)
        .expect("the resolved marketplace row is rendered");
    let row = &body[row_start..row_start + body[row_start..].find("</tr>").expect("row closes")];
    assert!(body.contains("Allowed"), "the group rule is effective");
    assert!(
        body.contains(&format!("Allowed through group {group}")),
        "the page explains the shared band that granted access: {body}"
    );
    assert!(
        row.contains("data-rule-id=\"\"") && row.contains("inherit"),
        "the marketplace has no personal override despite its shared grant: {row}"
    );
    assert!(
        !row.contains(&shared_rule_id),
        "the shared rule id is never exposed as a personal override control: {row}"
    );
    db.cleanup().await;
}

// The non-member group tabs answer three distinct questions about the same
// group: what directory claim feeds it, which project its people work in, and
// which workspace it entitles (a marketplace row of the Access tab).  Rendering
// them together proves their typed loaders keep the relationships aligned.
#[tokio::test(flavor = "multi_thread")]
async fn group_detail_tabs_render_mapping_project_and_marketplace_relationships() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let credentials = principal::provision(&db.pool).await;
    let app = App::new(&db.pool, credentials);
    let (_, _, _, group) = seed_member_with_group_and_project(&db.pool).await;
    sqlx::query(
        "INSERT INTO group_ad_mappings (ad_group, group_id, source)
         VALUES ('AD-SSR-Delivery', $1, 'dashboard')",
    )
    .bind(&group)
    .execute(&*db.pool)
    .await
    .expect("map directory group");
    set_group_marketplaces(
        &db.pool,
        &systemprompt_web_shared::GroupId::new(group.clone()),
        &[MarketplaceId::new("enterprise-demo")],
    )
    .await
    .expect("grant marketplace");

    for (tab, marker, meaning) in [
        ("mappings", "AD-SSR-Delivery", "directory mapping"),
        ("projects", "Core", "linked project"),
        (
            "access",
            "data-entity-type=\"marketplace\" data-entity-id=\"enterprise-demo\"",
            "workspace entitlement",
        ),
    ] {
        let path = format!("/admin/groups/{group}?tab={tab}");
        let (status, body) = app.call(Call::get(&path, Principal::Admin)).await;
        assert_eq!(status, StatusCode::OK, "{tab} tab: {body}");
        assert!(
            body.contains(marker),
            "{meaning} is rendered on the {tab} tab: {body}"
        );
    }
    let old_tab = format!("/admin/groups/{group}?tab=marketplaces");
    let (status, _) = app.call(Call::get(&old_tab, Principal::Admin)).await;
    assert_eq!(
        status,
        StatusCode::SEE_OTHER,
        "the retired Marketplaces tab redirects to Access"
    );
    db.cleanup().await;
}
