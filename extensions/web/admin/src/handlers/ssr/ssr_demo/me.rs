//! `/admin/demo/me` — the signed-in person's own demo activity.
//!
//! The only page in the Demo group a non-admin may open. It takes no user
//! parameter: the filter is built from the session's own `user_id`, so the page
//! cannot be pointed at anyone else by editing the URL.

use std::sync::Arc;

use axum::extract::{Extension, State};
use axum::response::Response;
use sqlx::PgPool;

use super::context::DemoMeContext;
use super::view::{logbook_row_view, mcp_tool_stat_view, skill_total_view};
use super::{ATTRIBUTION_NOTE, CHART_DAYS, kpi_strip};
use crate::error::AdminHtmlResult;
use crate::handlers::ssr::types::daily_count_chart;
use crate::repositories::demo::DemoFilter;
use crate::repositories::demo::kpis::{DemoKpis, get_demo_kpis};
use crate::repositories::demo::logbook::list_demo_logbook;
use crate::repositories::demo::mcp_tools::list_mcp_tool_stats;
use crate::repositories::demo::series::{list_mcp_tool_daily_series, list_skill_daily_series};
use crate::repositories::demo::skill_invocations::list_skill_totals;
use crate::templates::AdminTemplateEngine;
use crate::types::{MarketplaceContext, UserContext};

async fn build_page_json(pool: &PgPool, user_ctx: &UserContext) -> DemoMeContext {
    let filter = DemoFilter::for_user(user_ctx.user_id.clone());
    let (kpis, totals, stats, skill_series, tool_series, rows) = tokio::join!(
        get_demo_kpis(pool, &filter),
        list_skill_totals(pool, &filter),
        list_mcp_tool_stats(pool, &filter),
        list_skill_daily_series(pool, &filter, CHART_DAYS),
        list_mcp_tool_daily_series(pool, &filter, CHART_DAYS),
        list_demo_logbook(pool, &filter, false),
    );
    let kpis = kpis.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "demo kpi query failed");
        DemoKpis::default()
    });
    let skills: Vec<_> = totals
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "skill totals query failed");
            Vec::new()
        })
        .iter()
        .map(skill_total_view)
        .collect();
    let tools: Vec<_> = stats
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "mcp tool stats query failed");
            Vec::new()
        })
        .iter()
        .map(mcp_tool_stat_view)
        .collect();
    let skill_series = skill_series.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "skill daily series query failed");
        Vec::new()
    });
    let tool_series = tool_series.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "mcp tool daily series query failed");
        Vec::new()
    });
    let rows: Vec<_> = rows
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "demo logbook query failed");
            Vec::new()
        })
        .iter()
        .map(logbook_row_view)
        .collect();

    DemoMeContext {
        page: "demo-me",
        title: "My Demo Usage",
        subtitle: "The skills you ran, the MCP tools they called, and what the \
                   policy chain decided — yours only.",
        user_email: user_ctx.email.as_str().to_owned(),
        kpis: kpi_strip(&kpis),
        chart: daily_count_chart(
            &skill_series,
            "Your skill invocations per day",
            "accent",
            "You have not run a skill in this window.",
        ),
        tool_chart: daily_count_chart(
            &tool_series,
            "Your MCP tool calls per day",
            "success",
            "You have not called an MCP tool in this window.",
        ),
        has_skills: !skills.is_empty(),
        skills,
        has_tools: !tools.is_empty(),
        tools,
        has_rows: !rows.is_empty(),
        rows,
        attribution_note: ATTRIBUTION_NOTE,
    }
}

pub(crate) async fn demo_me_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
) -> AdminHtmlResult<Response> {
    let payload = build_page_json(&pool, &user_ctx).await;
    Ok(super::super::render_typed_page(
        &engine, "demo-me", &payload, &user_ctx, &mkt_ctx,
    ))
}
