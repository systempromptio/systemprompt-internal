//! `repositories::organizations::{seats, spend, metrics}` — seat accounting and
//! the limit both provisioning doors enforce, month-to-date spend against the
//! plan's cap, and the headline figures the enterprise console leads with.

use systemprompt_web_admin::repositories::organizations::{metrics, seats, spend};

use crate::fixtures::{
    OrgSpec, RequestSpec, insert_member, insert_org, insert_plan, insert_request, insert_user,
    insert_user_full, unclaimed_email, unique,
};
use crate::tempdb::TempDb;

#[tokio::test]
async fn count_active_seats_counts_only_active_members() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let org_id = unique("org");
    insert_org(&db.pool, &OrgSpec::active(&org_id, &org_id)).await;
    for (suffix, status) in [("one", "active"), ("two", "active"), ("three", "suspended")] {
        let user = insert_user_full(
            &db.pool,
            &unique("user"),
            &unclaimed_email(suffix),
            None,
            &["user".to_owned()],
            status,
        )
        .await;
        insert_member(&db.pool, &user, &org_id, "member").await;
    }

    let used = seats::count_active_seats(&db.pool, &org_id)
        .await
        .expect("count succeeds");

    assert_eq!(used, 2);
    db.cleanup().await;
}

#[tokio::test]
async fn get_seat_usage_errors_for_an_unknown_organization() {
    let Some(db) = TempDb::create().await else {
        return;
    };

    let err = seats::get_seat_usage(&db.pool, &unique("nope"))
        .await
        .expect_err("get_ reports an absent subject as an error");

    assert!(
        err.to_string().contains("Not found"),
        "unexpected error: {err}"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn get_seat_usage_prefers_the_negotiated_override_over_the_plan() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let plan_id = unique("plan");
    insert_plan(&db.pool, &plan_id, Some(5), None, 0).await;
    let org_id = unique("org");
    let mut spec = OrgSpec::active(&org_id, &org_id);
    spec.plan_id = Some(&plan_id);
    insert_org(&db.pool, &spec).await;
    sqlx::query("UPDATE organizations SET seat_limit_override = 50 WHERE id = $1")
        .bind(&org_id)
        .execute(&*db.pool)
        .await
        .expect("set the override");

    let usage = seats::get_seat_usage(&db.pool, &org_id)
        .await
        .expect("lookup succeeds");

    assert_eq!(usage.limit, Some(50));
    assert_eq!(usage.used, 0);
    assert!(!usage.is_full());
    db.cleanup().await;
}

#[tokio::test]
async fn assert_seat_available_passes_on_an_uncapped_plan() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let org_id = unique("org");
    insert_org(&db.pool, &OrgSpec::active(&org_id, &org_id)).await;

    seats::assert_seat_available(&db.pool, &org_id)
        .await
        .expect("a NULL seat limit is unlimited");
    db.cleanup().await;
}

#[tokio::test]
async fn find_spend_for_user_returns_none_without_a_capped_plan() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let org_id = unique("org");
    insert_org(&db.pool, &OrgSpec::active(&org_id, &org_id)).await;
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("nocap")).await;
    insert_member(&db.pool, &user, &org_id, "member").await;

    let found = spend::find_spend_for_user(&db.pool, &user)
        .await
        .expect("lookup succeeds");

    assert!(
        found.is_none(),
        "no cap applies, which every caller reads as 'no ceiling'"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn find_spend_for_user_sums_the_whole_organizations_month_to_date() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let plan_id = unique("plan");
    insert_plan(&db.pool, &plan_id, None, Some(1_000_000), 0).await;
    let org_id = unique("org");
    let mut spec = OrgSpec::active(&org_id, &org_id);
    spec.plan_id = Some(&plan_id);
    spec.name = "Capped Customer";
    insert_org(&db.pool, &spec).await;
    let asker = insert_user(&db.pool, &unique("user"), &unclaimed_email("asker")).await;
    let peer = insert_user(&db.pool, &unique("user"), &unclaimed_email("peer")).await;
    insert_member(&db.pool, &asker, &org_id, "member").await;
    insert_member(&db.pool, &peer, &org_id, "member").await;
    for owner in [&asker, &peer] {
        let request_id = unique("req");
        let mut req = RequestSpec::completed(&request_id, owner);
        req.cost_microdollars = 7_000;
        insert_request(&db.pool, &req).await;
    }

    let found = spend::find_spend_for_user(&db.pool, &asker)
        .await
        .expect("lookup succeeds")
        .expect("a capped plan produces a row");

    assert_eq!(found.name, "Capped Customer");
    assert_eq!(found.cap_microdollars, 1_000_000);
    assert_eq!(
        found.spent_microdollars, 14_000,
        "spend is the organization's, not the asker's"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn find_organization_metrics_returns_none_for_an_unknown_slug() {
    let Some(db) = TempDb::create().await else {
        return;
    };

    let found = metrics::find_organization_metrics(&db.pool, &unique("nope"))
        .await
        .expect("lookup succeeds");

    assert!(found.is_none());
    db.cleanup().await;
}

#[tokio::test]
async fn organization_metrics_state_margin_and_budget_health() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let plan_id = unique("plan");
    insert_plan(&db.pool, &plan_id, Some(10), Some(100_000), 400_000).await;
    let org_id = unique("org");
    let mut spec = OrgSpec::active(&org_id, &org_id);
    spec.plan_id = Some(&plan_id);
    insert_org(&db.pool, &spec).await;
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("spender")).await;
    insert_member(&db.pool, &user, &org_id, "owner").await;
    let request_id = unique("req");
    let mut req = RequestSpec::completed(&request_id, &user);
    req.cost_microdollars = 25_000;
    insert_request(&db.pool, &req).await;

    let found = metrics::find_organization_metrics(&db.pool, &org_id)
        .await
        .expect("lookup succeeds")
        .expect("the organization exists");

    assert_eq!(found.seats_used, 1);
    assert_eq!(found.seat_limit, Some(10));
    assert_eq!(found.requests_30d, 1);
    assert_eq!(found.cost_microdollars_mtd, 25_000);
    assert_eq!(found.revenue_microdollars, 400_000);
    assert_eq!(found.margin_microdollars(), 375_000);
    assert_eq!(found.budget_used_pct(), Some(25));
    db.cleanup().await;
}

#[tokio::test]
async fn organization_metrics_report_no_budget_health_without_a_cap() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let org_id = unique("org");
    insert_org(&db.pool, &OrgSpec::active(&org_id, &org_id)).await;

    let found = metrics::find_organization_metrics(&db.pool, &org_id)
        .await
        .expect("lookup succeeds")
        .expect("the organization exists");

    assert_eq!(
        found.budget_used_pct(),
        None,
        "an uncapped customer rendered at 0% would read as headroom, not as N/A"
    );
    assert_eq!(found.requests_30d, 0);
    db.cleanup().await;
}
