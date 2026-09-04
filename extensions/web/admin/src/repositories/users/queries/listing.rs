//! The user index listing and its filter options.

use sqlx::PgPool;
use systemprompt::identifiers::{Email, UserId};

use crate::types::UserSummary;

pub async fn list_users(pool: &PgPool) -> Result<Vec<UserSummary>, sqlx::Error> {
    list_users_filtered(pool, false).await
}

// Why: anonymous visitors are stored as ordinary user rows, so a roster that
// did not exclude them would present traffic as people. The flag exists so the
// page can still show them on request.
pub async fn list_users_filtered(
    pool: &PgPool,
    include_anonymous: bool,
) -> Result<Vec<UserSummary>, sqlx::Error> {
    sqlx::query_as!(
        UserSummary,
        r#"SELECT
                u.id AS "user_id!: UserId",
                COALESCE(u.display_name, u.full_name, u.name) AS display_name,
                u.email AS "email?: Email",
                u.roles AS "roles!: Vec<String>",
                (u.status = 'active') AS "is_active!",
                GREATEST(
                    COALESCE(MAX(p.created_at), u.created_at),
                    COALESCE(ua.last_ua, u.created_at),
                    COALESCE(mcp.last_mcp, u.created_at),
                    COALESCE(air.last_request, u.created_at)
                ) AS "last_active!",
                (COALESCE(COUNT(DISTINCT p.id), 0) + COALESCE(air.request_count, 0))::BIGINT AS "total_events!",
                (SELECT tool_name FROM plugin_usage_events p2
                 WHERE p2.user_id = u.id
                 ORDER BY created_at DESC LIMIT 1) AS last_tool,
                0::BIGINT AS "custom_skills_count!",
                NULL::TEXT AS preferred_client,
                COALESCE(COUNT(DISTINCT p.id) FILTER (WHERE p.event_type LIKE '%UserPromptSubmit%'), 0)::BIGINT AS "prompts!",
                COALESCE(COUNT(DISTINCT p.session_id), 0)::BIGINT AS "sessions!",
                (COALESCE(bytes.total_bytes, 0))::BIGINT AS "bytes!",
                COALESCE(ua.logins, 0)::BIGINT AS "logins!"
            FROM users u
            LEFT JOIN plugin_usage_events p ON p.user_id = u.id
            LEFT JOIN (
                SELECT user_id,
                       (COALESCE(SUM(content_input_bytes), 0) + COALESCE(SUM(content_output_bytes), 0))::BIGINT AS total_bytes
                FROM plugin_usage_daily GROUP BY user_id
            ) bytes ON bytes.user_id = u.id
            LEFT JOIN (
                SELECT user_id,
                       COUNT(*) FILTER (WHERE category = 'login')::BIGINT AS logins,
                       MAX(created_at) AS last_ua
                FROM user_activity GROUP BY user_id
            ) ua ON ua.user_id = u.id
            LEFT JOIN (
                SELECT user_id, MAX(created_at) AS last_mcp
                FROM mcp_tool_executions WHERE user_id IS NOT NULL
                GROUP BY user_id
            ) mcp ON mcp.user_id = u.id
            LEFT JOIN (
                SELECT user_id, MAX(created_at) AS last_request, COUNT(*)::BIGINT AS request_count
                FROM ai_requests GROUP BY user_id
            ) air ON air.user_id = u.id
            WHERE ($1::bool OR (NOT ('anonymous' = ANY(u.roles))
                              AND u.email NOT LIKE '%@anonymous.local'))
            GROUP BY u.id, u.created_at, u.name, u.display_name, u.full_name, u.email,
                     u.roles, u.status, bytes.total_bytes,
                     ua.logins, ua.last_ua, mcp.last_mcp, air.last_request,
                     air.request_count
            ORDER BY 6 DESC"#,
        include_anonymous,
    )
    .fetch_all(pool)
    .await
}

pub async fn count_anonymous_users(pool: &PgPool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT COUNT(*)::BIGINT AS "count!" FROM users u
           WHERE 'anonymous' = ANY(u.roles)
              OR u.email LIKE '%@anonymous.local'"#,
    )
    .fetch_one(pool)
    .await
}

pub async fn list_distinct_roles(pool: &PgPool) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT DISTINCT unnest(roles) AS "role!" FROM users
          WHERE NOT ('anonymous' = ANY(roles))
          ORDER BY 1"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| r.role)
        .filter(|r| !["anonymous", "a2a", "mcp", "service"].contains(&r.as_str()))
        .collect())
}
