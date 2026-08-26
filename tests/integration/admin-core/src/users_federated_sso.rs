//! `repositories::users::federated` — the SSO resolution order, the profile
//! connect/disconnect writes, and the seat check just-in-time provisioning
//! shares with the operator-created door.

use systemprompt_web_admin::repositories::users::federated::{
    FederatedClaims, LinkOutcome, delete_federated_identities_for_issuer, link_identity_to_user,
    resolve_federated_user,
};

use crate::fixtures::{
    OrgSpec, insert_federated_identity, insert_member, insert_org, insert_plan, insert_user,
    insert_user_full, unclaimed_email, unique,
};
use crate::tempdb::TempDb;

pub const ISSUER: &str = "https://idp.federated.test";

fn claims<'a>(external_sub: &'a str, email: &'a str) -> FederatedClaims<'a> {
    FederatedClaims {
        issuer: ISSUER,
        external_sub,
        email,
        display_name: "Federated Person",
    }
}

#[tokio::test]
async fn resolve_federated_user_returns_the_existing_mapping_first() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("mapped")).await;
    let sub = unique("sub");
    insert_federated_identity(&db.pool, ISSUER, &sub, &user).await;

    let resolved =
        resolve_federated_user(&db.pool, &claims(&sub, "other@elsewhere.test"), false, None)
            .await
            .expect("resolution succeeds")
            .expect("an existing mapping resolves without provisioning");

    assert_eq!(resolved.user_id, user, "the mapping wins over the email");
    db.cleanup().await;
}

#[tokio::test]
async fn resolve_federated_user_links_a_verified_email_to_an_active_local_account() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let email = unclaimed_email("merge");
    let user = insert_user(&db.pool, &unique("user"), &email).await;
    let sub = unique("sub");

    let resolved = resolve_federated_user(&db.pool, &claims(&sub, &email), false, None)
        .await
        .expect("resolution succeeds")
        .expect("an active local account is linked rather than duplicated");

    assert_eq!(resolved.user_id, user);
    let owner: Option<String> = sqlx::query_scalar(
        "SELECT user_id FROM federated_identities WHERE issuer = $1 AND external_sub = $2",
    )
    .bind(ISSUER)
    .bind(&sub)
    .fetch_optional(&*db.pool)
    .await
    .expect("mapping lookup succeeds");
    assert_eq!(
        owner.as_deref(),
        Some(user.as_str()),
        "the merge writes the mapping it resolved through"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn resolve_federated_user_matches_the_email_case_insensitively() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let email = unclaimed_email("case");
    let user = insert_user(&db.pool, &unique("user"), &email).await;
    let shouted = email.to_uppercase();

    let resolved = resolve_federated_user(&db.pool, &claims(&unique("sub"), &shouted), false, None)
        .await
        .expect("resolution succeeds")
        .expect("an upper-cased claim still finds the local account");

    assert_eq!(resolved.user_id, user);
    db.cleanup().await;
}

#[tokio::test]
async fn resolve_federated_user_ignores_an_inactive_local_account() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let email = unclaimed_email("inactive");
    insert_user_full(
        &db.pool,
        &unique("user"),
        &email,
        None,
        &["user".to_owned()],
        "inactive",
    )
    .await;

    let resolved = resolve_federated_user(&db.pool, &claims(&unique("sub"), &email), false, None)
        .await
        .expect("resolution succeeds");

    assert!(
        resolved.is_none(),
        "only an active account may be merged into; provisioning is off"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn resolve_federated_user_returns_none_when_provisioning_is_off() {
    let Some(db) = TempDb::create().await else {
        return;
    };

    let resolved = resolve_federated_user(
        &db.pool,
        &claims(&unique("sub"), &unclaimed_email("stranger")),
        false,
        None,
    )
    .await
    .expect("resolution succeeds");

    assert!(
        resolved.is_none(),
        "an unknown identity is not an error — the caller says 'ask an admin'"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn resolve_federated_user_provisions_when_asked_to() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let email = unclaimed_email("jit");
    let sub = unique("sub");

    let resolved = resolve_federated_user(&db.pool, &claims(&sub, &email), true, None)
        .await
        .expect("resolution succeeds")
        .expect("auto_provision mints the account");

    assert_eq!(resolved.email, email);
    assert_eq!(resolved.display_name, "Federated Person");
    assert_eq!(resolved.roles, vec!["user".to_owned()]);
    let status: String = sqlx::query_scalar("SELECT status FROM users WHERE id = $1")
        .bind(resolved.user_id.as_str())
        .fetch_one(&*db.pool)
        .await
        .expect("the provisioned user exists");
    assert_eq!(status, "active");
    db.cleanup().await;
}

#[tokio::test]
async fn resolve_federated_user_joins_the_organization_claiming_the_domain() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let domain = format!("{}.example", uuid::Uuid::new_v4().simple());
    let org_id = unique("org");
    let mut spec = OrgSpec::active(&org_id, &org_id);
    spec.email_domains = vec![domain.clone()];
    insert_org(&db.pool, &spec).await;
    let email = format!("newhire@{domain}");

    let resolved = resolve_federated_user(&db.pool, &claims(&unique("sub"), &email), true, None)
        .await
        .expect("resolution succeeds")
        .expect("auto_provision mints the account");

    let joined: Option<String> =
        sqlx::query_scalar("SELECT org_id FROM organization_members WHERE user_id = $1")
            .bind(resolved.user_id.as_str())
            .fetch_optional(&*db.pool)
            .await
            .expect("membership lookup succeeds");
    assert_eq!(joined.as_deref(), Some(org_id.as_str()));
    db.cleanup().await;
}

#[tokio::test]
async fn resolve_federated_user_refuses_to_provision_past_the_seat_limit() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let domain = format!("{}.example", uuid::Uuid::new_v4().simple());
    let plan_id = unique("plan");
    insert_plan(&db.pool, &plan_id, Some(1), None, 0).await;
    let org_id = unique("org");
    let mut spec = OrgSpec::active(&org_id, &org_id);
    spec.plan_id = Some(&plan_id);
    spec.email_domains = vec![domain.clone()];
    insert_org(&db.pool, &spec).await;
    let sitting = insert_user(&db.pool, &unique("user"), &format!("first@{domain}")).await;
    insert_member(&db.pool, &sitting, &org_id, "member").await;

    let refused = resolve_federated_user(
        &db.pool,
        &claims(&unique("sub"), &format!("second@{domain}")),
        true,
        None,
    )
    .await;

    let err = refused.expect_err("the last seat is taken, so JIT must be refused");
    assert!(
        err.to_string().contains("seat limit reached"),
        "a full plan is reported as a conflict the customer can act on: {err}"
    );
    let minted: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = $1")
        .bind(format!("second@{domain}"))
        .fetch_one(&*db.pool)
        .await
        .expect("count succeeds");
    assert_eq!(minted, 0, "the refused login must leave no orphan account");
    db.cleanup().await;
}

#[tokio::test]
async fn resolve_federated_user_provisions_with_the_callers_roles() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let desired = vec!["admin".to_owned(), "user".to_owned()];

    let resolved = resolve_federated_user(
        &db.pool,
        &claims(&unique("sub"), &unclaimed_email("odooadmin")),
        true,
        Some(&desired),
    )
    .await
    .expect("resolution succeeds")
    .expect("auto_provision mints the account");

    assert_eq!(
        resolved.roles, desired,
        "the identity provider's roles land on the freshly minted account"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn resolve_federated_user_refreshes_roles_on_a_returning_login() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("promoted")).await;
    let sub = unique("sub");
    insert_federated_identity(&db.pool, ISSUER, &sub, &user).await;
    let desired = vec!["admin".to_owned(), "user".to_owned()];

    let resolved =
        resolve_federated_user(&db.pool, &claims(&sub, "x@y.test"), false, Some(&desired))
            .await
            .expect("resolution succeeds")
            .expect("the mapping resolves");

    assert_eq!(resolved.roles, desired);
    let stored: Vec<String> = sqlx::query_scalar("SELECT roles FROM users WHERE id = $1")
        .bind(user.as_str())
        .fetch_one(&*db.pool)
        .await
        .expect("the user exists");
    assert_eq!(
        stored, desired,
        "a group change at the provider must land in users.roles at the next sign-in"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn resolve_federated_user_keeps_roles_when_the_caller_could_not_compute_them() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let email = unclaimed_email("unchanged");
    let user = insert_user_full(
        &db.pool,
        &unique("user"),
        &email,
        None,
        &["admin".to_owned(), "user".to_owned()],
        "active",
    )
    .await;
    let sub = unique("sub");
    insert_federated_identity(&db.pool, ISSUER, &sub, &user).await;

    let resolved = resolve_federated_user(&db.pool, &claims(&sub, &email), false, None)
        .await
        .expect("resolution succeeds")
        .expect("the mapping resolves");

    assert_eq!(
        resolved.roles,
        vec!["admin".to_owned(), "user".to_owned()],
        "a failed group lookup must never strip roles"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn link_identity_to_user_is_idempotent_for_the_same_owner() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("link")).await;
    let sub = unique("sub");

    let first = link_identity_to_user(&db.pool, ISSUER, &sub, &user)
        .await
        .expect("link succeeds");
    let second = link_identity_to_user(&db.pool, ISSUER, &sub, &user)
        .await
        .expect("re-link succeeds");

    assert_eq!(first, LinkOutcome::Linked);
    assert_eq!(second, LinkOutcome::Linked);
    db.cleanup().await;
}

#[tokio::test]
async fn link_identity_to_user_refuses_to_steal_another_users_mapping() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let owner = insert_user(&db.pool, &unique("user"), &unclaimed_email("owner")).await;
    let thief = insert_user(&db.pool, &unique("user"), &unclaimed_email("thief")).await;
    let sub = unique("sub");
    insert_federated_identity(&db.pool, ISSUER, &sub, &owner).await;

    let outcome = link_identity_to_user(&db.pool, ISSUER, &sub, &thief)
        .await
        .expect("link attempt succeeds");

    assert_eq!(outcome, LinkOutcome::AlreadyLinkedElsewhere);
    db.cleanup().await;
}

#[tokio::test]
async fn delete_federated_identities_for_issuer_reports_how_many_went() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("disconnect")).await;
    insert_federated_identity(&db.pool, ISSUER, &unique("sub"), &user).await;
    insert_federated_identity(&db.pool, ISSUER, &unique("sub"), &user).await;
    insert_federated_identity(&db.pool, "https://other.test", &unique("sub"), &user).await;

    let removed = delete_federated_identities_for_issuer(&db.pool, &user, ISSUER)
        .await
        .expect("delete succeeds");

    assert_eq!(removed, 2, "only this issuer's mappings are removed");
    let left: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM federated_identities WHERE user_id = $1")
            .bind(user.as_str())
            .fetch_one(&*db.pool)
            .await
            .expect("count succeeds");
    assert_eq!(left, 1, "the other issuer's mapping survives");
    db.cleanup().await;
}

#[tokio::test]
async fn delete_federated_identities_for_issuer_is_zero_when_nothing_matches() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("nothing")).await;

    let removed = delete_federated_identities_for_issuer(&db.pool, &user, ISSUER)
        .await
        .expect("delete succeeds");

    assert_eq!(removed, 0);
    db.cleanup().await;
}
