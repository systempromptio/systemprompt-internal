//! Paged conversation costs for the Sessions tab.
//!
//! `conversation_facts` holds one gateway-measured row per conversation —
//! tokens, cost and the model that served it — so every figure is what the
//! gateway metered, keyed on the conversation's last request.

use sqlx::PgPool;
use systemprompt::identifiers::{ContextId, SessionId, UserId};

use crate::util::time_range::TimeRange;

use super::SiteScope;

#[derive(Debug, Clone)]
pub struct SessionCostRow {
    pub context_id: ContextId,
    pub session_id: Option<SessionId>,
    pub user_id: UserId,
    pub model: Option<String>,
    pub total_cost_microdollars: i64,
    pub request_count: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_session_costs_paged(
    pool: &PgPool,
    range: TimeRange,
    scope: &SiteScope,
    limit: i64,
    offset: i64,
) -> Result<(Vec<SessionCostRow>, i64), sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT
            f.context_id AS "context_id!: ContextId",
            f.client_session_id AS "session_id?: SessionId",
            f.user_id AS "user_id!: UserId",
            f.model,
            f.cost_microdollars AS "cost!",
            f.request_count AS "request_count!",
            f.input_tokens AS "input_tokens!",
            f.output_tokens AS "output_tokens!",
            f.cache_read_tokens AS "cache_read!",
            f.last_at AS "updated_at!",
            COUNT(*) OVER ()::BIGINT AS "total!"
        FROM conversation_facts f
        WHERE f.last_at >= $1 AND f.last_at < $2
          AND ($3::TEXT[] IS NULL OR f.user_id = ANY($3))
          AND ($4::TEXT IS NULL OR f.user_id = $4)
        ORDER BY f.cost_microdollars DESC, f.last_at DESC
        LIMIT $5 OFFSET $6
        "#,
        range.from,
        range.to,
        scope.scope.as_sql(),
        scope.user_id_str(),
        limit,
        offset,
    )
    .fetch_all(pool)
    .await?;

    let total = rows.first().map_or(0, |r| r.total);
    Ok((
        rows.into_iter()
            .map(|r| SessionCostRow {
                context_id: r.context_id,
                session_id: r.session_id,
                user_id: r.user_id,
                model: r.model,
                total_cost_microdollars: r.cost,
                request_count: r.request_count,
                input_tokens: r.input_tokens,
                output_tokens: r.output_tokens,
                cache_read_tokens: r.cache_read,
                updated_at: r.updated_at,
            })
            .collect(),
        total,
    ))
}
