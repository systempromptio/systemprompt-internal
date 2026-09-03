//! `/admin/entities/skills` — invocation counts per skill, with a
//! session-attributed cost estimate. See
//! `repositories::analytics::skills` for what "estimate" means here.

mod context;

use std::sync::Arc;

use crate::error::{AdminError, AdminHtmlResult};
use crate::repositories::analytics::skills;
use crate::templates::AdminTemplateEngine;
use crate::types::{MarketplaceContext, UserContext};
use axum::extract::{Extension, State};
use axum::response::Response;
use sqlx::PgPool;

use context::{PageStat, SkillRowView, SkillsPageContext};

fn microdollars_to_usd(microdollars: i64) -> f64 {
    microdollars as f64 / 1_000_000.0
}

async fn build_page_json(pool: &PgPool) -> SkillsPageContext {
    let rows = skills::list_skill_usage_stats(pool)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "list_skill_usage_stats failed");
            Vec::new()
        });

    let mut skill_rows = Vec::with_capacity(rows.len());
    let mut total_invocations = 0i64;
    for row in &rows {
        let estimate = skills::get_skill_cost_estimate(pool, &row.skill_id)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, skill_id = %row.skill_id, "get_skill_cost_estimate failed");
                skills::SkillCostEstimate::default()
            });
        total_invocations += row.invocation_count;
        skill_rows.push(SkillRowView {
            skill_id: row.skill_id.clone(),
            invocation_count: row.invocation_count,
            distinct_users: row.distinct_users,
            first_used_at: row.first_used_at.map(|d| d.to_rfc3339()),
            last_used_at: row.last_used_at.map(|d| d.to_rfc3339()),
            estimated_session_count: estimate.session_count,
            estimated_request_count: estimate.request_count,
            estimated_tokens: estimate.total_input_tokens + estimate.total_output_tokens,
            estimated_cost_usd: microdollars_to_usd(estimate.total_cost_microdollars),
        });
    }

    SkillsPageContext {
        page: "skill-usage",
        title: "Skill Usage",
        skills: skill_rows,
        page_stats: vec![
            PageStat {
                value: rows.len() as i64,
                label: "Skills used",
            },
            PageStat {
                value: total_invocations,
                label: "Invocations",
            },
        ],
    }
}

pub(crate) async fn skill_usage_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
) -> AdminHtmlResult<Response> {
    if !user_ctx.is_admin {
        return Err(AdminError::Forbidden("Admin access required.".to_owned()).into());
    }
    let payload = build_page_json(&pool).await;
    Ok(super::render_typed_page(
        &engine, "skill-usage", &payload, &user_ctx, &mkt_ctx,
    ))
}
