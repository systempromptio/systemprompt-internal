//! The two CTEs every scoped query is built on: `membership` and
//! `request_scope`.
//!
//! Two bound parameters decide them: `$1` is the container kind (`'group'` or
//! `'project'`) and `$2` is whether attribution is exclusive. Nothing is
//! interpolated, so every statement built through `scoped_query` and
//! `scoped_query_as` is a static string the `sqlx` macros verify against the
//! live schema, and a caller's tail starts its own placeholders at `$3`.
//!
//! `membership` answers "who is in this container". Member attribution reads
//! full membership: a person in two groups appears under both, so the group
//! totals overlap and deliberately do not sum to the instance total.
//! Exclusive attribution reads `user_scope_defaults` instead, where each
//! person holds one primary group and one primary project, so the totals
//! partition the instance.
//!
//! `request_scope` answers "which container does this request belong to",
//! and is what every spend figure joins on. Under exclusive attribution it
//! reads `ai_request_scopes`, the scope stamped by trigger when the request
//! landed — so moving a person never rewrites their history, and a request
//! no primary covered at the time keeps a NULL scope and belongs to the
//! unattributed bucket, which every instance-wide breakdown reports rather
//! than drops. Under member attribution it is the request joined to full
//! membership, the overlapping view, and its `source` is `'member'`.

use sqlx::PgPool;
use systemprompt::identifiers::UserId;

use super::{Attribution, ScopeTarget};

// Why: the id every instance-wide breakdown files traffic under when it
// attributes to nobody — a rejected request with no user, a non-user actor, or
// a person no primary container covers.
pub const UNATTRIBUTED: &str = "unattributed";

#[macro_export]
macro_rules! scoped_query {
    ($tail:literal, $($args:tt)*) => {
        sqlx::query!(
            "WITH membership AS (
                 SELECT ug.user_id, ug.group_id AS scope_id
                   FROM user_groups ug
                  WHERE $1::TEXT = 'group' AND NOT $2::BOOLEAN
                 UNION
                 SELECT pm.user_id, pm.project_id AS scope_id
                   FROM user_projects pm
                  WHERE $1::TEXT = 'project' AND NOT $2::BOOLEAN
                 UNION
                 SELECT d.user_id, d.primary_group_id AS scope_id
                   FROM user_scope_defaults d
                  WHERE $1::TEXT = 'group' AND $2::BOOLEAN
                    AND d.primary_group_id IS NOT NULL
                 UNION
                 SELECT d.user_id, d.primary_project_id AS scope_id
                   FROM user_scope_defaults d
                  WHERE $1::TEXT = 'project' AND $2::BOOLEAN
                    AND d.primary_project_id IS NOT NULL
             ), request_scope AS (
                 SELECT s.request_id, s.user_id,
                        CASE WHEN $1::TEXT = 'group' THEN s.group_id
                             ELSE s.project_id END AS scope_id,
                        s.source
                   FROM ai_request_scopes s
                  WHERE $2::BOOLEAN
                 UNION ALL
                 SELECT r.id AS request_id, r.user_id, m.scope_id, 'member' AS source
                   FROM ai_requests r
                   JOIN membership m ON m.user_id = r.user_id
                  WHERE NOT $2::BOOLEAN AND r.actor_kind = 'user'
             ) " + $tail,
            $($args)*
        )
    };
}

pub async fn list_scope_user_ids(
    pool: &PgPool,
    target: ScopeTarget<'_>,
    attribution: Attribution,
) -> Result<Vec<UserId>, sqlx::Error> {
    let rows = scoped_query!(
        r#"SELECT m.user_id AS "user_id!: UserId" FROM membership m WHERE m.scope_id = $3"#,
        target.kind().as_str(),
        attribution.is_exclusive(),
        target.id()
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| row.user_id).collect())
}

pub async fn get_subject_scope(
    pool: &PgPool,
    request: &super::ScopeRequest,
) -> Result<super::SubjectScope, sqlx::Error> {
    let started = std::time::Instant::now();
    let result = resolve_subject_scope(pool, request).await;
    tracing::debug!(
        scope_ms = started.elapsed().as_secs_f64() * 1000.0,
        "dashboard scope resolved"
    );
    result
}

async fn resolve_subject_scope(
    pool: &PgPool,
    request: &super::ScopeRequest,
) -> Result<super::SubjectScope, sqlx::Error> {
    let groups = request.group_filter();
    if groups.is_none() && request.project.is_none() {
        return Ok(super::SubjectScope::All);
    }

    // Why: a scoped caller's group restriction remains in force even when the
    // query names a project; a query string cannot widen visibility.
    let ids = sqlx::query_scalar!(
        r#"SELECT u.id AS "id!"
           FROM users u
           WHERE ($1::TEXT[] IS NULL OR EXISTS (
                     SELECT 1 FROM user_groups ug
                     WHERE ug.user_id = u.id AND ug.group_id = ANY($1)))
             AND ($2::TEXT IS NULL OR EXISTS (
                     SELECT 1 FROM project_members pm
                     WHERE pm.user_id = u.id AND pm.project_id = $2))"#,
        groups.as_deref(),
        request.project.as_deref()
    )
    .fetch_all(pool)
    .await?;

    Ok(super::SubjectScope::Users(ids))
}
