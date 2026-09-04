//! `/admin/demo/tools` — MCP tool usage and the governance verdicts on it.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::{Extension, State};
use axum::response::Response;
use sqlx::PgPool;

use super::context::DemoToolsContext;
use super::view::{
    McpToolStatView, ServerCardView, ToolVerdictTotals, format_demo_cost, matrix_view,
    mcp_tool_stat_view,
};
use super::{ATTRIBUTION_NOTE, CHART_DAYS, tool_kpi_strip};
use crate::error::{AdminError, AdminHtmlResult};
use crate::handlers::ssr::types::daily_count_chart;
use crate::repositories::demo::kpis::{DemoKpis, get_demo_kpis};
use crate::repositories::demo::mcp_tools::{list_mcp_tool_stats, list_user_mcp_tool_matrix};
use crate::repositories::demo::series::list_mcp_tool_daily_series;
use crate::repositories::demo::{DemoFilter, UsageMatrix};
use crate::templates::AdminTemplateEngine;
use crate::types::{MarketplaceContext, UserContext};

#[derive(Default)]
struct ServerTotals {
    tool_count: i64,
    call_count: i64,
    failure_count: i64,
    denied: i64,
    held: i64,
    cost_microdollars: i64,
}

fn server_cards(tools: &[McpToolStatView], costs: &[i64]) -> Vec<ServerCardView> {
    let mut by_server: BTreeMap<&str, ServerTotals> = BTreeMap::new();
    for (tool, cost) in tools.iter().zip(costs) {
        let entry = by_server.entry(tool.server.as_str()).or_default();
        entry.tool_count += 1;
        entry.call_count += tool.call_count;
        entry.failure_count += tool.failure_count;
        entry.denied += tool.denied;
        entry.held += tool.held;
        entry.cost_microdollars += *cost;
    }
    by_server
        .into_iter()
        .filter(|(_, t)| t.call_count > 0)
        .map(|(server, t)| ServerCardView {
            server: server.to_owned(),
            tool_count: t.tool_count,
            call_count: t.call_count,
            failure_count: t.failure_count,
            denied: t.denied,
            held: t.held,
            cost_display: format_demo_cost(t.cost_microdollars),
        })
        .collect()
}

async fn build_page_json(pool: &PgPool) -> DemoToolsContext {
    let filter = DemoFilter::all_users();
    let (kpis, stats, series, matrix) = tokio::join!(
        get_demo_kpis(pool, &filter),
        list_mcp_tool_stats(pool, &filter),
        list_mcp_tool_daily_series(pool, &filter, CHART_DAYS),
        list_user_mcp_tool_matrix(pool, &filter),
    );
    let kpis = kpis.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "demo kpi query failed");
        DemoKpis::default()
    });
    let stats = stats.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "mcp tool stats query failed");
        Vec::new()
    });
    let series = series.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "mcp tool daily series query failed");
        Vec::new()
    });
    let matrix = matrix.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "mcp tool matrix query failed");
        UsageMatrix::default()
    });

    let costs: Vec<i64> = stats.iter().map(|s| s.cost_microdollars).collect();
    let verdicts = stats
        .iter()
        .fold(ToolVerdictTotals::default(), |mut acc, s| {
            acc.allowed += s.allowed;
            acc.denied += s.denied;
            acc.held += s.held;
            acc.approved += s.approved;
            acc
        });
    let tools: Vec<_> = stats.iter().map(mcp_tool_stat_view).collect();
    let servers = server_cards(&tools, &costs);

    DemoToolsContext {
        page: "demo-tools",
        title: "MCP Tool Usage",
        subtitle: "Every MCP tool call the demo made, what the policy chain said \
                   about it, and what it cost.",
        kpis: tool_kpi_strip(&kpis, &verdicts),
        chart: daily_count_chart(
            &series,
            "MCP tool calls, last 14 days",
            "accent",
            "No MCP tool calls in this window.",
        ),
        has_servers: !servers.is_empty(),
        servers,
        has_tools: !tools.is_empty(),
        tools,
        matrix: matrix_view(&matrix),
        attribution_note: ATTRIBUTION_NOTE,
    }
}

pub(crate) async fn demo_tools_page(
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
        "demo-tools",
        &payload,
        &user_ctx,
        &mkt_ctx,
    ))
}
