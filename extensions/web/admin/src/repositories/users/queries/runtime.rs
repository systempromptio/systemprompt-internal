//! Live runtime aggregates per user, for the control centre.
//!
//! Sourced from the governance spine (`ai_requests`) and the access tokens
//! that reach it — the only per-user activity this deployment records.

use sqlx::PgPool;
use systemprompt::identifiers::UserId;

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct UserRuntimeAggregate {
    pub user_id: UserId,
    pub newest_token_used_at: Option<chrono::DateTime<chrono::Utc>>,
    pub lifetime_tokens: i64,
}

pub async fn list_user_runtime_aggregates(
    pool: &PgPool,
) -> Result<Vec<UserRuntimeAggregate>, sqlx::Error> {
    sqlx::query_as!(
        UserRuntimeAggregate,
        r#"
        SELECT
            u.id AS "user_id!: UserId",
            ak.newest_used AS "newest_token_used_at?",
            COALESCE(req.lifetime_tokens, 0)::BIGINT AS "lifetime_tokens!"
        FROM users u
        LEFT JOIN (
            SELECT user_id, SUM(tokens_used)::BIGINT AS lifetime_tokens
            FROM ai_requests GROUP BY user_id
        ) req ON req.user_id = u.id
        LEFT JOIN (
            SELECT user_id, MAX(last_used_at) AS newest_used
            FROM user_api_keys WHERE revoked_at IS NULL GROUP BY user_id
        ) ak ON ak.user_id = u.id
        WHERE NOT ('anonymous' = ANY(u.roles))
          AND u.email NOT LIKE '%@anonymous.local'
        "#,
    )
    .fetch_all(pool)
    .await
}

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct UserRuntimeDetail {
    pub requests: i64,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub last_request_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn get_user_runtime_detail(
    pool: &PgPool,
    user_id: &UserId,
) -> Result<UserRuntimeDetail, sqlx::Error> {
    let totals = sqlx::query!(
        r#"
        SELECT
            COUNT(*)::BIGINT AS "requests!",
            COALESCE(SUM(input_tokens), 0)::BIGINT AS "tokens_in!",
            COALESCE(SUM(output_tokens), 0)::BIGINT AS "tokens_out!",
            MAX(created_at) AS "last_request_at?"
        FROM ai_requests WHERE user_id = $1
        "#,
        user_id.as_str()
    )
    .fetch_one(pool)
    .await?;

    Ok(UserRuntimeDetail {
        requests: totals.requests,
        tokens_in: totals.tokens_in,
        tokens_out: totals.tokens_out,
        last_request_at: totals.last_request_at,
    })
}
