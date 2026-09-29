//! Raw-SQL row builders.
//!
//! Every test owns its own database, but that database is not empty: the web
//! extension's migrations seed a house organization and three demo tenants
//! with thirty days of traffic. Fixtures therefore mint their own ids, and
//! assertions either name those ids or use a historical time window the demo
//! rows cannot reach.

use chrono::{DateTime, TimeZone, Utc};
use sqlx::PgPool;
use systemprompt::identifiers::UserId;
use systemprompt_web_admin::repositories::scope::SubjectScope;

// A window far enough in the past that no migration-seeded row falls in it.
#[must_use]
pub fn ancient_window() -> (DateTime<Utc>, DateTime<Utc>) {
    (at(2001, 3, 1, 0), at(2001, 4, 1, 0))
}

#[must_use]
pub fn at(year: i32, month: u32, day: u32, hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, 0, 0)
        .single()
        .expect("fixed timestamp is unambiguous")
}

#[must_use]
pub fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4().simple())
}

#[must_use]
pub fn user_id(raw: &str) -> UserId {
    UserId::new(raw.to_owned())
}

pub async fn insert_user(pool: &PgPool, id: &str) {
    insert_user_with_roles(pool, id, &["user".to_owned()]).await;
}

pub async fn insert_user_with_roles(pool: &PgPool, id: &str, roles: &[String]) {
    sqlx::query(
        "INSERT INTO users (id, name, email, display_name, status, email_verified, roles)
         VALUES ($1, $2, $2, $3, 'active', TRUE, $4)",
    )
    .bind(id)
    .bind(format!("{id}@example.test"))
    .bind(format!("User {id}"))
    .bind(roles.to_vec())
    .execute(pool)
    .await
    .expect("insert user");
}

pub async fn insert_plan(
    pool: &PgPool,
    id: &str,
    price: i64,
    cap: Option<i64>,
    seats: Option<i32>,
) {
    sqlx::query(
        "INSERT INTO plans (id, name, description, seat_limit,
                            monthly_cost_cap_microdollars, monthly_price_microdollars)
         VALUES ($1, $1, '', $2, $3, $4)",
    )
    .bind(id)
    .bind(seats)
    .bind(cap)
    .bind(price)
    .execute(pool)
    .await
    .expect("insert plan");
}

pub async fn insert_org(pool: &PgPool, id: &str, plan_id: Option<&str>) {
    insert_org_with_status(pool, id, plan_id, "active").await;
}

pub async fn insert_org_with_status(pool: &PgPool, id: &str, plan_id: Option<&str>, status: &str) {
    sqlx::query(
        "INSERT INTO organizations (id, slug, name, plan_id, status)
         VALUES ($1, $1, $2, $3, $4)",
    )
    .bind(id)
    .bind(format!("Org {id}"))
    .bind(plan_id)
    .bind(status)
    .execute(pool)
    .await
    .expect("insert organization");
}

pub async fn add_member(pool: &PgPool, user: &str, org: &str) {
    sqlx::query("INSERT INTO organization_members (user_id, org_id) VALUES ($1, $2)")
        .bind(user)
        .bind(org)
        .execute(pool)
        .await
        .expect("insert organization member");
}

// One `ai_requests` row. Defaults describe a completed, costed request; each
// test overrides only the columns its assertion reads.
pub struct RequestSeed<'a> {
    pub id: &'a str,
    pub user_id: &'a str,
    pub provider: Option<&'a str>,
    pub model: Option<&'a str>,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cache_read_tokens: i32,
    pub cost_microdollars: i64,
    pub status: &'a str,
    pub created_at: DateTime<Utc>,
}

impl<'a> RequestSeed<'a> {
    #[must_use]
    pub fn new(id: &'a str, user_id: &'a str, created_at: DateTime<Utc>) -> Self {
        Self {
            id,
            user_id,
            provider: Some("anthropic"),
            model: Some("claude-sonnet-4-5-20250929"),
            input_tokens: 100,
            output_tokens: 20,
            cache_read_tokens: 0,
            cost_microdollars: 1_000,
            status: "completed",
            created_at,
        }
    }
}

pub async fn insert_request(pool: &PgPool, seed: &RequestSeed<'_>) {
    sqlx::query(
        "INSERT INTO ai_requests
            (id, request_id, user_id, context_id, provider, model, input_tokens, output_tokens,
             tokens_used, cache_read_tokens, cost_microdollars, status,
             actor_kind, actor_id, created_at, updated_at)
         VALUES ($1, $1, $2, md5($2)::uuid, $3, $4, $5, $6, $7, $8, $9, $10, 'user', $2, $11, $11)",
    )
    .bind(seed.id)
    .bind(seed.user_id)
    .bind(seed.provider)
    .bind(seed.model)
    .bind(seed.input_tokens)
    .bind(seed.output_tokens)
    .bind(seed.input_tokens + seed.output_tokens)
    .bind(seed.cache_read_tokens)
    .bind(seed.cost_microdollars)
    .bind(seed.status)
    .bind(seed.created_at)
    .execute(pool)
    .await
    .expect("insert ai request");
}

// Registers the catalog row `access_control_rules` foreign-keys against.
pub async fn insert_acl_entity(pool: &PgPool, entity_type: &str, entity_id: &str, default: bool) {
    sqlx::query(
        "INSERT INTO access_control_entities (entity_type, entity_id, default_included, source)
         VALUES ($1, $2, $3, 'test')
         ON CONFLICT (entity_type, entity_id) DO UPDATE SET default_included = EXCLUDED.default_included",
    )
    .bind(entity_type)
    .bind(entity_id)
    .bind(default)
    .execute(pool)
    .await
    .expect("insert access-control entity");
}

#[expect(
    clippy::too_many_arguments,
    reason = "fixture mirrors the row's columns"
)]
pub async fn insert_env_var(
    pool: &PgPool,
    user: &str,
    plugin: &str,
    name: &str,
    value: &str,
    secret: bool,
) -> String {
    let id = unique("env");
    sqlx::query(
        "INSERT INTO plugin_env_vars (id, user_id, plugin_id, var_name, var_value, is_secret)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&id)
    .bind(user)
    .bind(plugin)
    .bind(name)
    .bind(value)
    .bind(secret)
    .execute(pool)
    .await
    .expect("insert plugin env var");
    id
}

pub async fn count_rows(pool: &PgPool, sql: &'static str, arg: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(sql)
        .bind(arg)
        .fetch_one(pool)
        .await
        .expect("count rows")
}

// Writes `services/<rel>` under `dir`, creating parents.
pub fn write_services_file(dir: &std::path::Path, rel: &str, contents: &str) {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create services subdirectory");
    }
    std::fs::write(path, contents).expect("write services file");
}

// Put `user` in `project` (creating the project), or, with `None`, take them
// out of every project. The customer report scopes by resolved user id now,
// so membership is what the fixture has to establish.
pub async fn set_project(pool: &PgPool, user: &str, project: Option<&str>) {
    sqlx::query("DELETE FROM project_members WHERE user_id = $1")
        .bind(user)
        .execute(pool)
        .await
        .expect("clear project members");
    let Some(project) = project else {
        return;
    };
    sqlx::query(
        "INSERT INTO projects (id, name, source) VALUES ($1, $1, 'dashboard')
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(project)
    .execute(pool)
    .await
    .expect("insert project");
    sqlx::query(
        "INSERT INTO project_members (project_id, user_id, source) VALUES ($1, $2, 'manual')
         ON CONFLICT DO NOTHING",
    )
    .bind(project)
    .bind(user)
    .execute(pool)
    .await
    .expect("insert project member");
}

// The users in one project, as a bound scope.
pub async fn project_scope(pool: &PgPool, project: &str) -> SubjectScope {
    let ids: Vec<String> =
        sqlx::query_scalar("SELECT user_id FROM project_members WHERE project_id = $1")
            .bind(project)
            .fetch_all(pool)
            .await
            .expect("project scope");
    SubjectScope::Users(ids)
}
