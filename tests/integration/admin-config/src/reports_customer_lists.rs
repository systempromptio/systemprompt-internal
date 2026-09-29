//! `repositories::reports::customer` — the per-user, per-project, and
//! per-model tables under the header figures.

use systemprompt_web_admin::repositories::reports::customer::{
    list_customer_month_models, list_customer_month_projects, list_customer_month_users,
};

use systemprompt_web_admin::repositories::scope::SubjectScope;

use crate::fixtures::{
    RequestSeed, ancient_window, at, insert_request, insert_user, project_scope, set_project,
    unique,
};
use crate::tempdb::TempDb;

// Two users on different projects, both of whom made a request inside the
// historical window, plus one request outside it.
struct Scenario {
    heavy: String,
    light: String,
}

async fn seed(pool: &sqlx::PgPool) -> Scenario {
    let heavy = unique("u-heavy");
    let light = unique("u-light");
    for user in [&heavy, &light] {
        insert_user(pool, user).await;
    }
    set_project(pool, &heavy, Some("commerce")).await;
    set_project(pool, &light, Some("core")).await;

    let mut seed = RequestSeed::new("r-heavy-1", &heavy, at(2001, 3, 5, 9));
    seed.input_tokens = 1_000;
    seed.output_tokens = 200;
    seed.cache_read_tokens = 50;
    seed.cost_microdollars = 900_000;
    insert_request(pool, &seed).await;

    let mut seed = RequestSeed::new("r-heavy-2", &heavy, at(2001, 3, 6, 9));
    seed.model = Some("claude-opus-4-5-20251101");
    seed.input_tokens = 500;
    seed.output_tokens = 100;
    seed.status = "failed";
    insert_request(pool, &seed).await;

    let mut seed = RequestSeed::new("r-light-1", &light, at(2001, 3, 7, 9));
    seed.input_tokens = 10;
    seed.output_tokens = 2;
    insert_request(pool, &seed).await;

    let mut seed = RequestSeed::new("r-outside", &light, at(2001, 5, 1, 9));
    seed.input_tokens = 999_999;
    insert_request(pool, &seed).await;

    Scenario { heavy, light }
}

#[tokio::test]
async fn list_customer_month_users_orders_by_tokens_consumed() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let scenario = seed(&db.pool).await;
    let (from, to) = ancient_window();

    let rows = list_customer_month_users(&db.pool, &SubjectScope::All, from, to)
        .await
        .expect("list users");

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].email, format!("{}@example.test", scenario.heavy));
    assert_eq!(rows[0].requests, 2);
    assert_eq!(rows[0].distinct_models, 2);
    assert_eq!(rows[1].requests, 1);

    db.cleanup().await;
}

#[tokio::test]
async fn list_customer_month_users_carries_each_users_project() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let scenario = seed(&db.pool).await;
    let (from, to) = ancient_window();

    let rows = list_customer_month_users(&db.pool, &SubjectScope::All, from, to)
        .await
        .expect("list users");

    let light = rows
        .iter()
        .find(|r| r.email.starts_with(&scenario.light))
        .expect("the lighter user is listed");
    assert_eq!(light.project.as_deref(), Some("core"));
    let heavy = rows
        .iter()
        .find(|r| r.email.starts_with(&scenario.heavy))
        .expect("the heavier user is listed");
    assert_eq!(heavy.project.as_deref(), Some("commerce"));

    db.cleanup().await;
}

#[tokio::test]
async fn list_customer_month_users_omits_members_with_no_activity() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let _scenario = seed(&db.pool).await;
    let idle = unique("u-idle");
    insert_user(&db.pool, &idle).await;
    set_project(&db.pool, &idle, Some("commerce")).await;
    let (from, to) = ancient_window();

    let rows = list_customer_month_users(&db.pool, &SubjectScope::All, from, to)
        .await
        .expect("list users");

    assert!(!rows.iter().any(|r| r.email.starts_with(&idle)));

    db.cleanup().await;
}

#[tokio::test]
async fn list_customer_month_projects_aggregates_members_and_tokens() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let _scenario = seed(&db.pool).await;
    let (from, to) = ancient_window();

    let rows = list_customer_month_projects(&db.pool, &SubjectScope::All, from, to)
        .await
        .expect("list projects");

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].project, "commerce", "heaviest project leads");
    assert_eq!(rows[0].members, 1);
    assert_eq!(rows[0].requests, 2);
    assert_eq!(rows[1].project, "core");

    db.cleanup().await;
}

#[tokio::test]
async fn list_customer_month_models_groups_by_provider_and_model() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    seed(&db.pool).await;
    let (from, to) = ancient_window();

    let rows = list_customer_month_models(&db.pool, &SubjectScope::All, from, to)
        .await
        .expect("list models");

    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.provider == "anthropic"));
    let sonnet = rows
        .iter()
        .find(|r| r.model == "claude-sonnet-4-5-20250929")
        .expect("sonnet row");
    assert_eq!(sonnet.requests, 2, "both members used the default model");
    assert_eq!(sonnet.cache_read_tokens, 50);

    db.cleanup().await;
}

#[tokio::test]
async fn customer_month_lists_are_empty_under_the_nothing_scope() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let _scenario = seed(&db.pool).await;
    let (from, to) = ancient_window();

    let users = list_customer_month_users(&db.pool, &SubjectScope::Users(Vec::new()), from, to)
        .await
        .expect("list users");
    let projects =
        list_customer_month_projects(&db.pool, &SubjectScope::Users(Vec::new()), from, to)
            .await
            .expect("list projects");
    let models = list_customer_month_models(&db.pool, &SubjectScope::Users(Vec::new()), from, to)
        .await
        .expect("list models");

    assert!(users.is_empty());
    assert!(projects.is_empty());
    assert!(models.is_empty());

    db.cleanup().await;
}

#[tokio::test]
async fn customer_month_lists_narrow_to_one_project() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let scenario = seed(&db.pool).await;
    let (from, to) = ancient_window();
    let scope = project_scope(&db.pool, "core").await;

    let users = list_customer_month_users(&db.pool, &scope, from, to)
        .await
        .expect("list users");
    let models = list_customer_month_models(&db.pool, &scope, from, to)
        .await
        .expect("list models");

    assert_eq!(users.len(), 1);
    assert!(users[0].email.starts_with(&scenario.light));
    assert_eq!(
        models.len(),
        1,
        "the core user only touched the default model"
    );
    assert_eq!(models[0].requests, 1);

    db.cleanup().await;
}
