//! `/admin/entities/skills` — invocation counts per skill, with AI usage
//! attributed by the window rule in `repositories::demo::attribution`.

mod context;

use std::sync::Arc;

use crate::error::{AdminError, AdminHtmlResult};
use crate::repositories::demo::filter::DemoFilter;
use crate::repositories::demo::skill_invocations;
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
    let filter = DemoFilter::all_users();
    let rows = skill_invocations::list_skill_totals(pool, &filter)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "list_skill_totals failed");
            Vec::new()
        });

    let total_invocations = rows.iter().map(|r| r.invocation_count).sum();
    let skill_rows = rows
        .iter()
        .map(|row| SkillRowView {
            skill: row.skill.clone(),
            invocation_count: row.invocation_count,
            distinct_users: row.distinct_users,
            first_used_at: row.first_used_at.map(|d| d.to_rfc3339()),
            last_used_at: row.last_used_at.map(|d| d.to_rfc3339()),
            attributed_request_count: row.request_count,
            attributed_tokens: row.total_tokens,
            attributed_cost_usd: microdollars_to_usd(row.cost_microdollars),
        })
        .collect();

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
        &engine,
        "skill-usage",
        &payload,
        &user_ctx,
        &mkt_ctx,
    ))
}
