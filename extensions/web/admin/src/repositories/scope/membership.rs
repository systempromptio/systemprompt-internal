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
    let explicit_group = request.group.is_some();
    let explicit_project = request.project.is_some();
    // Why: a scoped caller naming one container asks about that container
    // alone; the other dimension must not silently intersect it away.
    let (groups, projects) = match (request.is_scoped(), explicit_group, explicit_project) {
        (true, true, false) => (request.group_filter(), None),
        (true, false, true) => (None, request.project_filter()),
        _ => (request.group_filter(), request.project_filter()),
    };
    if groups.is_none() && projects.is_none() {
        return Ok(super::SubjectScope::All);
    }

    // Why: an explicit `?group=` / `?project=` filter is a question about the
    // container's spend, so it also admits anyone with a request stamped to
    // it — a person who has since moved keeps showing under the container
    // their history was attributed to. The visibility narrowing a scoped
    // caller gets without asking stays on current membership, so leaving a
    // group ends what its peers can see of you. A scoped caller's own id is
    // always admitted: their view can narrow to nothing but themselves.
    //
    // A console caller filtering on both containers asks for their
    // intersection; a scoped caller's containers are a union — every person
    // their marketplaces reach, by whichever container reaches them.
    let union = request.is_scoped() && !explicit_group && !explicit_project;
    let ids = sqlx::query_scalar!(
        r#"WITH in_groups AS (
               SELECT u.id
               FROM users u
               WHERE $1::TEXT[] IS NOT NULL AND (
                     EXISTS (SELECT 1 FROM user_groups ug
                             WHERE ug.user_id = u.id AND ug.group_id = ANY($1))
                  OR ($3::BOOLEAN AND EXISTS (
                             SELECT 1 FROM ai_request_scopes s
                             WHERE s.user_id = u.id AND s.group_id = ANY($1))))
           ),
           in_projects AS (
               SELECT u.id
               FROM users u
               WHERE $2::TEXT[] IS NOT NULL AND (
                     EXISTS (SELECT 1 FROM user_projects pm
                             WHERE pm.user_id = u.id AND pm.project_id = ANY($2))
                  OR ($4::BOOLEAN AND EXISTS (
                             SELECT 1 FROM ai_request_scopes s
                             WHERE s.user_id = u.id AND s.project_id = ANY($2))))
           )
           SELECT u.id AS "id!"
           FROM users u
           WHERE u.id = $6::TEXT
              OR CASE
                   WHEN $5::BOOLEAN THEN
                        u.id IN (SELECT id FROM in_groups)
                     OR u.id IN (SELECT id FROM in_projects)
                   ELSE
                        ($1::TEXT[] IS NULL OR u.id IN (SELECT id FROM in_groups))
                    AND ($2::TEXT[] IS NULL OR u.id IN (SELECT id FROM in_projects))
                 END"#,
        groups.as_deref(),
        projects.as_deref(),
        explicit_group,
        explicit_project,
        union,
        request.self_id()
    )
    .fetch_all(pool)
    .await?;

    Ok(super::SubjectScope::Users(ids))
}
