//! `/admin/demo` — the merged governance logbook and the three demo scenarios.

use std::sync::Arc;

use axum::extract::{Extension, State};
use axum::response::Response;
use sqlx::PgPool;

use super::context::DemoLogbookContext;
use super::view::logbook_row_view;
use super::{ATTRIBUTION_NOTE, logbook_kpi_strip, scenario_cards};
use crate::error::{AdminError, AdminHtmlResult};
use crate::repositories::demo::DemoFilter;
use crate::repositories::demo::kpis::{DemoKpis, get_demo_kpis};
use crate::repositories::demo::logbook::list_demo_logbook;
use crate::templates::AdminTemplateEngine;
use crate::types::{MarketplaceContext, UserContext};

async fn build_page_json(pool: &PgPool) -> DemoLogbookContext {
    let filter = DemoFilter::all_users();
    let (kpis, rows) = tokio::join!(
        get_demo_kpis(pool, &filter),
        list_demo_logbook(pool, &filter, false),
    );
    let kpis = kpis.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "demo kpi query failed");
        DemoKpis::default()
    });
    let rows = rows.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "demo logbook query failed");
        Vec::new()
    });
    let rows: Vec<_> = rows.iter().map(logbook_row_view).collect();

    DemoLogbookContext {
        page: "demo-logbook",
        title: "Demo Logbook",
        subtitle: "Every skill invocation, MCP tool call, policy decision and \
                   approval the demo produced, in the order it happened.",
        kpis: logbook_kpi_strip(&kpis),
        scenarios: scenario_cards(&kpis),
        has_rows: !rows.is_empty(),
        rows,
        attribution_note: ATTRIBUTION_NOTE,
    }
}

pub(crate) async fn demo_logbook_page(
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
        "demo-logbook",
        &payload,
        &user_ctx,
        &mkt_ctx,
    ))
}
