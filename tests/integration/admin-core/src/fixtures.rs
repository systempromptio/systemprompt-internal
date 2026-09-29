//! Row builders for the admin repository suite.
//!
//! IMPORTANT: the throwaway database is **not empty**.
//! `install_extension_schemas` runs the web extension's migrations, and several
//! of those seed rows — `022_organizations_backfill` creates the `house`
//! organization (marked the platform tenant by `024`) and `009` seeds the
//! `Default` department. Tests therefore assert on *their own* rows and on
//! deltas, never on absolute table counts or on a list being empty.
//! (`025_demo_organizations` used to add three demo customers with ten users
//! and ~1080 `ai_requests` on top of that; it has been removed.)
//!
//! Every id is suffixed with a fresh UUID so two tests sharing a database
//! (they do not today, but the fixtures must not depend on that) cannot
//! collide, and so a `find_`/`get_` miss is genuinely a miss rather than a
//! collision with seeded data.

use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use systemprompt::identifiers::{ContextId, UserId};
use systemprompt_web_admin::repositories::scope::SubjectScope;
use systemprompt_web_admin::util::time_range::{TimeRange, TimeRangePreset};
use systemprompt_web_shared::{GroupId, ProjectId};

// A window that starts after the newest seeded `ai_requests` row (migration
// 025 writes none newer than a minute old) and runs into the future, so a
// windowed query sees only what the test inserted.
pub fn narrow_window() -> TimeRange {
    let now = Utc::now();
    TimeRange {
        from: now - Duration::seconds(30),
        to: now + Duration::hours(1),
        preset: TimeRangePreset::Custom,
        rejected_bounds: false,
    }
}

// An hour either side of now, for tables with no seeded rows at all.
pub fn wide_window() -> TimeRange {
    let now = Utc::now();
    TimeRange {
        from: now - Duration::hours(1),
        to: now + Duration::hours(1),
        preset: TimeRangePreset::Custom,
        rejected_bounds: false,
    }
}

// A fresh, collision-proof id fragment.
pub fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4().simple())
}

// An email in a domain no seeded organization claims.
pub fn unclaimed_email(local: &str) -> String {
    format!("{local}@{}.example", uuid::Uuid::new_v4().simple())
}

// Insert an active user with the `user` role.
//
// `users.email` is unique and must already be lowercase and trimmed; the
// email doubles as the name for fixture brevity.
pub async fn insert_user(pool: &PgPool, id: &str, email: &str) -> UserId {
    insert_user_full(pool, id, email, Some(email), &["user".to_owned()], "active").await
}

#[expect(
    clippy::too_many_arguments,
    reason = "fixture mirrors the row's columns"
)]
pub async fn insert_user_full(
    pool: &PgPool,
    id: &str,
    email: &str,
    display_name: Option<&str>,
    roles: &[String],
    status: &str,
) -> UserId {
    // Why: the matrix resolver reads the composed services tree for the
    // entity catalog; an empty tree is the neutral one for these tests.
    static SERVICES: std::sync::Once = std::sync::Once::new();
    SERVICES.call_once(|| {
        if systemprompt::loader::ServicesBootstrap::is_initialized() {
            return;
        }
        let directory = tempfile::tempdir().expect("services fixture");
        let path = directory.path().join("config.yaml");
        std::fs::write(&path, "{}").expect("write services fixture");
        systemprompt::loader::ServicesBootstrap::init_from_path(&path)
            .expect("install services fixture");
    });

    sqlx::query(
        "INSERT INTO users (id, name, email, display_name, status, email_verified, roles)
         VALUES ($1, $2, $3, $4, $5, true, $6)",
    )
    .bind(id)
    .bind(email)
    .bind(email)
    .bind(display_name)
    .bind(status)
    .bind(roles)
    .execute(pool)
    .await
    .expect("insert user");
    UserId::new(id.to_owned())
}

pub async fn set_department(pool: &PgPool, user_id: &UserId, department: &str) {
    let _ = department;
    sqlx::query(
        "INSERT INTO user_profile_ext (user_id) VALUES ($1)
         ON CONFLICT (user_id) DO NOTHING",
    )
    .bind(user_id.as_str())
    .execute(pool)
    .await
    .expect("set department");
}

// Insert a `user_contexts` row.
//
// `session_id` carries a foreign key to `user_sessions`, so pass `None`
// unless that session row already exists.
pub async fn insert_context(
    pool: &PgPool,
    context_id: &str,
    user_id: &UserId,
    session_id: Option<&str>,
    name: &str,
) {
    sqlx::query(
        "INSERT INTO user_contexts (context_id, user_id, session_id, name) VALUES ($1, $2, $3, $4)",
    )
    .bind(context_id)
    .bind(user_id.as_str())
    .bind(session_id)
    .bind(name)
    .execute(pool)
    .await
    .expect("insert user context");
}

// A `plugin_session_summaries` row — the hook-side half of a session, which
// `find_session_header` full-outer-joins against the `ai_requests` rollup.
pub struct SummarySpec<'a> {
    pub session_id: &'a str,
    pub user_id: &'a UserId,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub tool_uses: i64,
    pub prompts: i64,
    pub errors: i64,
    pub model: Option<&'a str>,
    pub status: Option<&'a str>,
    pub ai_title: Option<&'a str>,
}

impl<'a> SummarySpec<'a> {
    // A summary that started an hour ago and has not ended.
    pub fn open(session_id: &'a str, user_id: &'a UserId) -> Self {
        Self {
            session_id,
            user_id,
            started_at: Some(Utc::now() - Duration::hours(1)),
            ended_at: None,
            tool_uses: 0,
            prompts: 0,
            errors: 0,
            model: None,
            status: None,
            ai_title: None,
        }
    }
}

pub async fn insert_summary(pool: &PgPool, spec: &SummarySpec<'_>) {
    sqlx::query(
        "INSERT INTO plugin_session_summaries
             (id, session_id, user_id, started_at, ended_at, tool_uses, prompts, errors,
              model, status, ai_title, total_events)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $6 + $7)",
    )
    .bind(unique("summary"))
    .bind(spec.session_id)
    .bind(spec.user_id.as_str())
    .bind(spec.started_at)
    .bind(spec.ended_at)
    .bind(spec.tool_uses)
    .bind(spec.prompts)
    .bind(spec.errors)
    .bind(spec.model)
    .bind(spec.status)
    .bind(spec.ai_title)
    .execute(pool)
    .await
    .expect("insert session summary");
}

// `ai_requests.session_id` carries a foreign key to `user_sessions`, so a
// request with a session needs the session row first.
pub async fn insert_session(pool: &PgPool, session_id: &str, user_id: &UserId) {
    sqlx::query("INSERT INTO user_sessions (session_id, user_id) VALUES ($1, $2)")
        .bind(session_id)
        .bind(user_id.as_str())
        .execute(pool)
        .await
        .expect("insert user session");
}

pub const LEGACY_CONTEXT_ID: &str = "00000000-0000-0000-0000-4c4547414359";

pub struct RequestSpec<'a> {
    // Owned so a caller may pass `&unique("req")` directly: a borrowed id would
    // dangle the moment the `let` binding holding the spec ended.
    pub id: String,
    pub user_id: &'a UserId,
    pub session_id: Option<&'a str>,
    // The client's own session id (Claude Code's uuid) that a gateway row
    // carries beside the bridge session; the trace queries fold on it.
    pub client_session_id: Option<&'a str>,
    pub trace_id: Option<&'a str>,
    pub context_id: Option<&'a str>,
    pub provider: &'a str,
    pub model: &'a str,
    pub status: &'a str,
    pub input_tokens: i32,
    pub output_tokens: i32,
    // The provider's own total. Settable because it is not always
    // `input + output`: it counts cache reads too. `None` keeps the derived
    // `input + output` every existing caller relies on.
    pub tokens_used: Option<i32>,
    // Reasoning is billed inside `output_tokens`, and the cache counts are
    // disjoint from `input_tokens`. A fixture that cannot set them cannot
    // prove a reader reports them beside the total rather than into it.
    pub reasoning_tokens: Option<i32>,
    pub cache_read_tokens: Option<i32>,
    pub cache_creation_tokens: Option<i32>,
    pub cost_microdollars: i64,
    pub latency_ms: i32,
    pub created_at: DateTime<Utc>,
}

impl<'a> RequestSpec<'a> {
    pub fn completed(id: &str, user_id: &'a UserId) -> Self {
        Self {
            id: id.to_owned(),
            user_id,
            session_id: None,
            client_session_id: None,
            trace_id: None,
            // ai_requests.context_id is NOT NULL; this is core's own sentinel
            // for a row that belongs to no known context.
            context_id: Some(LEGACY_CONTEXT_ID),
            provider: "anthropic",
            model: "claude-test-model",
            status: "completed",
            input_tokens: 100,
            output_tokens: 20,
            tokens_used: None,
            reasoning_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            cost_microdollars: 5_000,
            latency_ms: 250,
            created_at: Utc::now(),
        }
    }
}

// A payload row offering tools: the conversation view classifies a lone
// request with no tools as a side call, so a test that wants a real turn
// (opening prompt, search) inserts this beside the request.
//
// Core 0.59.0 stores a tool list once in `ai_tool_catalogs` and has the
// payload reference its digest, so the catalogue row comes first and the
// payload names it.
pub async fn insert_offered_tools(pool: &PgPool, request_id: &str) {
    let tools = serde_json::json!([{"name": "bash"}]);
    let sha256 = format!("{:064x}", 0xba5eba11u64);
    sqlx::query(
        "INSERT INTO ai_tool_catalogs (sha256, tools) VALUES ($1, $2) \
         ON CONFLICT (sha256) DO NOTHING",
    )
    .bind(&sha256)
    .bind(&tools)
    .execute(pool)
    .await
    .expect("insert tool catalogue");

    sqlx::query(
        "INSERT INTO ai_request_payloads (ai_request_id, offered_tools_sha256) VALUES ($1, $2)",
    )
    .bind(request_id)
    .bind(&sha256)
    .execute(pool)
    .await
    .expect("insert ai request payload");
}

pub async fn insert_request(pool: &PgPool, spec: &RequestSpec<'_>) {
    sqlx::query(
        "INSERT INTO ai_requests (
             id, request_id, user_id, session_id, trace_id, context_id,
             provider, model, input_tokens, output_tokens, tokens_used,
             reasoning_tokens, cache_read_tokens, cache_creation_tokens,
             cost_microdollars, latency_ms, status, actor_kind, actor_id,
             created_at, updated_at, client_session_id)
         VALUES ($1, $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                 $14, $15, $16, 'user', $2, $17, $17, $18)",
    )
    .bind(&spec.id)
    .bind(spec.user_id.as_str())
    .bind(spec.session_id)
    .bind(spec.trace_id)
    .bind(spec.context_id)
    .bind(spec.provider)
    .bind(spec.model)
    .bind(spec.input_tokens)
    .bind(spec.output_tokens)
    .bind(
        spec.tokens_used
            .unwrap_or(spec.input_tokens + spec.output_tokens),
    )
    .bind(spec.reasoning_tokens)
    .bind(spec.cache_read_tokens)
    .bind(spec.cache_creation_tokens)
    .bind(spec.cost_microdollars)
    .bind(spec.latency_ms)
    .bind(spec.status)
    .bind(spec.created_at)
    .bind(spec.client_session_id)
    .execute(pool)
    .await
    .expect("insert ai_request");
}

pub struct DecisionSpec<'a> {
    pub id: String,
    pub user_id: &'a UserId,
    pub session_id: &'a str,
    pub tool_name: &'a str,
    pub agent_id: Option<&'a str>,
    pub agent_scope: Option<&'a str>,
    pub decision: &'a str,
    pub policy: &'a str,
    pub reason: &'a str,
    pub plugin_id: Option<&'a str>,
    pub trace_id: Option<&'a str>,
    pub entity_type: Option<&'a str>,
    pub created_at: DateTime<Utc>,
}

impl<'a> DecisionSpec<'a> {
    pub fn allow(id: &str, user_id: &'a UserId, session_id: &'a str) -> Self {
        Self {
            id: id.to_owned(),
            user_id,
            session_id,
            tool_name: "Bash",
            agent_id: None,
            agent_scope: None,
            decision: "allow",
            policy: "scope_check",
            reason: "within scope",
            plugin_id: None,
            trace_id: None,
            entity_type: None,
            created_at: Utc::now(),
        }
    }
}

pub async fn insert_decision(pool: &PgPool, spec: &DecisionSpec<'_>) {
    sqlx::query(
        "INSERT INTO governance_decisions (
             id, user_id, session_id, context_id, tool_name, agent_id, agent_scope,
             decision, policy, reason, plugin_id, actor_kind, actor_id, created_at,
             trace_id, evaluated_rules)
         VALUES ($1, $2, $3, $11, $4, $5, $6, $7, $8, $9, $12, 'user', $2, $10,
                 $13,
                 CASE WHEN $14::TEXT IS NULL THEN '[]'::JSONB
                      ELSE jsonb_build_object('entity_type', $14::TEXT) END)",
    )
    .bind(&spec.id)
    .bind(spec.user_id.as_str())
    .bind(spec.session_id)
    .bind(spec.tool_name)
    .bind(spec.agent_id)
    .bind(spec.agent_scope)
    .bind(spec.decision)
    .bind(spec.policy)
    .bind(spec.reason)
    .bind(spec.created_at)
    .bind(LEGACY_CONTEXT_ID)
    .bind(spec.plugin_id)
    .bind(spec.trace_id)
    .bind(spec.entity_type)
    .execute(pool)
    .await
    .expect("insert governance decision");
}

pub struct EventSpec<'a> {
    pub id: String,
    pub user_id: &'a UserId,
    pub session_id: &'a str,
    pub event_type: &'a str,
    pub tool_name: Option<&'a str>,
    pub plugin_id: Option<&'a str>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl<'a> EventSpec<'a> {
    pub fn tool_use(id: &str, user_id: &'a UserId, session_id: &'a str) -> Self {
        Self {
            id: id.to_owned(),
            user_id,
            session_id,
            event_type: "claude_code_PostToolUse",
            tool_name: Some("Bash"),
            plugin_id: None,
            metadata: serde_json::Value::Null,
            created_at: Utc::now(),
        }
    }
}

pub async fn insert_event(pool: &PgPool, spec: &EventSpec<'_>) {
    sqlx::query(
        "INSERT INTO plugin_usage_events
             (id, user_id, session_id, event_type, tool_name, plugin_id, metadata, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(&spec.id)
    .bind(spec.user_id.as_str())
    .bind(spec.session_id)
    .bind(spec.event_type)
    .bind(spec.tool_name)
    .bind(spec.plugin_id)
    .bind(&spec.metadata)
    .bind(spec.created_at)
    .execute(pool)
    .await
    .expect("insert plugin usage event");
}

pub async fn insert_activity(
    pool: &PgPool,
    id: &str,
    user_id: &UserId,
    category: &str,
    action: &str,
) {
    sqlx::query(
        "INSERT INTO user_activity (id, user_id, category, action, entity_name, description)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(user_id.as_str())
    .bind(category)
    .bind(action)
    .bind("fixture-entity")
    .bind("fixture activity")
    .execute(pool)
    .await
    .expect("insert user activity");
}

// Insert an access-control grant, creating the catalog row the FK requires.
#[expect(
    clippy::too_many_arguments,
    reason = "fixture mirrors the row's columns"
)]
pub async fn insert_acl_rule(
    pool: &PgPool,
    entity_type: &str,
    entity_id: &str,
    rule_type: &str,
    rule_value: &str,
    access: &str,
) {
    sqlx::query(
        "INSERT INTO access_control_entities (entity_type, entity_id, default_included, source)
         VALUES ($1, $2, false, 'fixture')
         ON CONFLICT (entity_type, entity_id) DO NOTHING",
    )
    .bind(entity_type)
    .bind(entity_id)
    .execute(pool)
    .await
    .expect("insert access control entity");

    sqlx::query(
        "INSERT INTO access_control_rules
             (id, entity_type, entity_id, rule_type, rule_value, access)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(unique("rule"))
    .bind(entity_type)
    .bind(entity_id)
    .bind(rule_type)
    .bind(rule_value)
    .bind(access)
    .execute(pool)
    .await
    .expect("insert access control rule");
}

pub async fn insert_federated_identity(
    pool: &PgPool,
    issuer: &str,
    external_sub: &str,
    user_id: &UserId,
) {
    sqlx::query(
        "INSERT INTO federated_identities (issuer, external_sub, user_id) VALUES ($1, $2, $3)",
    )
    .bind(issuer)
    .bind(external_sub)
    .bind(user_id.as_str())
    .execute(pool)
    .await
    .expect("insert federated identity");
}

pub fn new_context_id() -> String {
    ContextId::generate().to_string()
}

pub async fn set_project(pool: &PgPool, user_id: &UserId, project: Option<&str>) {
    sqlx::query("DELETE FROM project_members WHERE user_id = $1")
        .bind(user_id.as_str())
        .execute(pool)
        .await
        .expect("clear project members");
    sqlx::query("DELETE FROM group_members WHERE user_id = $1")
        .bind(user_id.as_str())
        .execute(pool)
        .await
        .expect("clear group members");
    let Some(project) = project else {
        return;
    };
    insert_project(pool, project, project).await;
    insert_project_member(pool, project, user_id, "manual").await;
    insert_group(pool, project, project).await;
    insert_group_member(pool, project, user_id, "manual").await;
}

pub async fn project_scope(pool: &PgPool, project: &str) -> SubjectScope {
    let ids: Vec<String> =
        sqlx::query_scalar("SELECT user_id FROM project_members WHERE project_id = $1")
            .bind(project)
            .fetch_all(pool)
            .await
            .expect("project scope");
    SubjectScope::Users(ids)
}

pub async fn insert_group(pool: &PgPool, id: impl AsRef<str>, name: &str) {
    sqlx::query(
        "INSERT INTO groups (id, name, source) VALUES ($1, $2, 'dashboard')
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(id.as_ref())
    .bind(name)
    .execute(pool)
    .await
    .expect("insert group");
}

pub async fn insert_group_member(
    pool: &PgPool,
    group_id: impl AsRef<str>,
    user_id: &UserId,
    source: &str,
) {
    sqlx::query(
        "INSERT INTO group_members (group_id, user_id, source) VALUES ($1, $2, $3)
         ON CONFLICT DO NOTHING",
    )
    .bind(group_id.as_ref())
    .bind(user_id.as_str())
    .bind(source)
    .execute(pool)
    .await
    .expect("insert group member");
}

pub async fn insert_project(pool: &PgPool, id: impl AsRef<str>, name: &str) {
    sqlx::query(
        "INSERT INTO projects (id, name, source) VALUES ($1, $2, 'dashboard')
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(id.as_ref())
    .bind(name)
    .execute(pool)
    .await
    .expect("insert project");
}

pub async fn insert_project_member(
    pool: &PgPool,
    project_id: impl AsRef<str>,
    user_id: &UserId,
    source: &str,
) {
    sqlx::query(
        "INSERT INTO project_members (project_id, user_id, source) VALUES ($1, $2, $3)
         ON CONFLICT DO NOTHING",
    )
    .bind(project_id.as_ref())
    .bind(user_id.as_str())
    .bind(source)
    .execute(pool)
    .await
    .expect("insert project member");
}

pub(crate) fn unique_group(prefix: &str) -> GroupId {
    GroupId::new(unique(prefix))
}

pub(crate) fn unique_project(prefix: &str) -> ProjectId {
    ProjectId::new(unique(prefix))
}

pub(crate) async fn insert_message(
    pool: &PgPool,
    request_id: &str,
    seq: i32,
    role: &str,
    body: &str,
) {
    sqlx::query(
        "INSERT INTO ai_request_messages (id, request_id, role, content, sequence_number)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(unique("msg"))
    .bind(request_id)
    .bind(role)
    .bind(body)
    .bind(seq)
    .execute(pool)
    .await
    .expect("insert ai request message");
}
