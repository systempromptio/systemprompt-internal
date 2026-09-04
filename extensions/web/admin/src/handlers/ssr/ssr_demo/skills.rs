//! `/admin/demo/skills` — which skills get used, by whom, and what they cost.

use std::sync::Arc;

use axum::extract::{Extension, State};
use axum::response::Response;
use sqlx::PgPool;

use super::context::DemoSkillsContext;
use super::view::{matrix_view, skill_total_view, user_total_views};
use super::{ATTRIBUTION_NOTE, CHART_DAYS, skill_kpi_strip};
use crate::error::{AdminError, AdminHtmlResult};
use crate::handlers::ssr::types::daily_count_chart;
use crate::repositories::demo::kpis::{DemoKpis, get_demo_kpis};
use crate::repositories::demo::series::list_skill_daily_series;
use crate::repositories::demo::skill_invocations::list_skill_totals;
use crate::repositories::demo::skill_matrix::list_user_skill_matrix;
use crate::repositories::demo::{DemoFilter, UsageMatrix};
use crate::templates::AdminTemplateEngine;
use crate::types::{MarketplaceContext, UserContext};

async fn build_page_json(pool: &PgPool) -> DemoSkillsContext {
    let filter = DemoFilter::all_users();
    let (kpis, totals, series, matrix) = tokio::join!(
        get_demo_kpis(pool, &filter),
        list_skill_totals(pool, &filter),
        list_skill_daily_series(pool, &filter, CHART_DAYS),
        list_user_skill_matrix(pool, &filter),
    );
    let kpis = kpis.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "demo kpi query failed");
        DemoKpis::default()
    });
    let totals = totals.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "skill totals query failed");
        Vec::new()
    });
    let series = series.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "skill daily series query failed");
        Vec::new()
    });
    let matrix = matrix.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "skill matrix query failed");
        UsageMatrix::default()
    });

    let skills: Vec<_> = totals.iter().map(skill_total_view).collect();
    let user_totals = user_total_views(&matrix);

    DemoSkillsContext {
        page: "demo-skills",
        title: "Skill Adoption",
        subtitle: "Skill invocations recorded by the Claude Code hooks, with the \
                   AI usage attributed to each one.",
        kpis: skill_kpi_strip(&kpis, skills.len() as i64, matrix.rows.len() as i64),
        chart: daily_count_chart(
            &series,
            "Skill invocations, last 14 days",
            "accent",
            "No skill invocations in this window.",
        ),
        has_skills: !skills.is_empty(),
        skills,
        matrix: matrix_view(&matrix),
        has_user_totals: !user_totals.is_empty(),
        user_totals,
        attribution_note: ATTRIBUTION_NOTE,
    }
}

pub(crate) async fn demo_skills_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
) -> AdminHtmlResult<Response> {
    if !user_ctx.is_admin {
        return Err(AdminError::Forbidden("Admin access required.".to_owned()).into());
    }
    let payload = build_page_json(&pool).await;
    Ok(super::super::render_typed_page(
        &engine,
        "demo-skills",
        &payload,
        &user_ctx,
        &mkt_ctx,
    ))
}
