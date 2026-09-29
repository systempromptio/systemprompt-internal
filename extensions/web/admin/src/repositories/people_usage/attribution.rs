//! How a container's window was attributed: one row per `source` on the
//! request-scope stamp.
//!
//! The detail pages show this beside the spend so a reader can see that the
//! figure was fixed request by request — `primary` is the person's default
//! at the time, `header` a per-request override — rather than re-derived
//! from whoever is a member today. Read under exclusive attribution only:
//! the member view has no stamp to report and would answer `member` for
//! every row.

use serde::Serialize;
use sqlx::PgPool;

use crate::repositories::scope::ScopeQuery;

/// One attribution source and the slice of the window it decided.
#[derive(Debug, Clone, Serialize)]
pub struct AttributionSourceRow {
    pub source: String,
    pub requests: i64,
    pub cost_microdollars: i64,
}

pub async fn list_scope_attribution_sources(
    pool: &PgPool,
    q: &ScopeQuery<'_>,
) -> Result<Vec<AttributionSourceRow>, sqlx::Error> {
    let rows = crate::scoped_query!(
        r#"SELECT rs.source AS "source!",
                  COUNT(*)::BIGINT AS "requests!",
                  COALESCE(SUM(r.cost_microdollars), 0)::BIGINT AS "cost_microdollars!"
           FROM request_scope rs
           JOIN ai_requests r ON r.id = rs.request_id
           WHERE rs.scope_id = $3
             AND r.created_at >= NOW() - make_interval(days => $4)
           GROUP BY 1
           ORDER BY 2 DESC, 1"#,
        q.kind().as_str(),
        q.attribution.is_exclusive(),
        q.id(),
        q.window_days
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| AttributionSourceRow {
            source: row.source,
            requests: row.requests,
            cost_microdollars: row.cost_microdollars,
        })
        .collect())
}
