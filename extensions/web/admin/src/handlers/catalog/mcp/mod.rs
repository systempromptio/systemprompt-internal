//! `/admin/mcp` and `/admin/mcp/{id}` — the MCP servers, what they are serving,
//! and who is on them.
//!
//! These are the only admin pages that read the MCP runtime tables, so they are
//! the only place an operator can see that a declared server has never
//! connected, or that traffic is arriving under a name nothing declares. Both
//! states are silent everywhere else.
//!
//! Every figure is for one window (`?preset=` / `?from=&to=`, 24 hours by
//! default), and the export of either page carries the same window, so the
//! file answers the question the page was asked.

mod columns;
mod detail;
mod fleet;
mod rows;
mod sections;
mod view;
mod window;

pub(crate) use fleet::fleet_rows;
pub use rows::status_of;
pub(crate) use view::McpServerRow;

use std::sync::Arc;
use systemprompt::identifiers::McpServerId;

use axum::extract::{Extension, Path, Query, State};
use axum::response::Response;
use serde::Deserialize;
use sqlx::PgPool;

use crate::error::{AdminError, AdminHtmlResult};
use crate::export::ExportView;
use crate::handlers::shared;
use crate::handlers::ssr::list_view::TimeRangeContext;
use crate::handlers::ssr::types::BreadcrumbView;
use crate::templates::AdminTemplateEngine;
use crate::types::{ENTITY_MCP_SERVER, MarketplaceContext, UserContext};
use crate::util::time_range::TimeRange;

use super::super::ssr::ssr_helpers::render_typed_page;
use super::sorting::{direction, matches, sort_headers};
use super::view::assignment_counts_by_type;
use crate::handlers::ssr::page::Page;
use fleet::load_runtime;
use rows::{BASE_URL, RowInputs};
use view::{McpDetailData, McpPageData};
use window::{pairs_to_query, time_range_context, window_label, window_of, window_pairs};

#[derive(Debug, Default, Deserialize)]
pub(crate) struct McpListQuery {
    pub sort: Option<String>,
    pub dir: Option<String>,
    // Why: filtering is a server round trip, not a class toggle — the count in
    // the toolbar, the empty state and the URL then cannot disagree with the
    // rows on screen.
    pub q: Option<String>,
    pub preset: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct McpDetailQuery {
    pub page: Option<i64>,
    pub preset: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    // Why: the "Who gets this" panel's "Why?" person.
    pub why: Option<String>,
}

fn console_only(user_ctx: &UserContext) -> AdminHtmlResult<()> {
    if user_ctx.is_console {
        return Ok(());
    }
    Err(AdminError::Forbidden("Admin access required.".to_owned()).into())
}

pub(crate) async fn mcp_servers_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
    Query(query): Query<McpListQuery>,
) -> AdminHtmlResult<Response> {
    console_only(&user_ctx)?;
    let range = window_of(query.preset, query.from, query.to);
    let fleet = fleet_rows(&pool, &user_ctx.roles, range).await?;
    let mut servers = fleet.rows;

    let search = query.q.unwrap_or_default();
    // Why: the tiles count the fleet, not the filtered view. A filter that
    // moved "alive now" would make the number mean two different things
    // depending on what was typed in the box above it.
    let kpis = columns::kpis(&servers);
    servers.retain(|s| matches(&[&s.id, &s.description, &s.server_type], &search));
    let sort_key = query.sort.unwrap_or_else(|| "calls".to_owned());
    let sort_dir = direction(query.dir.as_deref());
    rows::sort_rows(&mut servers, &sort_key, sort_dir);

    let mut pairs = vec![("q", Some(search.clone()))];
    pairs.extend(window_pairs(&range));
    let preserved_query = pairs_to_query(&pairs);

    let unconfigured_count = servers.iter().filter(|s| !s.configured).count();
    let page = McpPageData {
        page: "mcp",
        title: "MCP servers",
        subtitle: "Every tool server this instance declares, what it is serving right now, and who may reach it.",
        breadcrumbs: vec![BreadcrumbView::current("MCP servers")],
        window_label: window_label(&range),
        heartbeat_label: format!(
            "alive = spoke within {} minutes",
            crate::repositories::overview::liveness::HEARTBEAT_INTERVAL_SECS * 2 / 60
        ),
        time_range: time_range_context(&range, BASE_URL),
        window_from: range.from.to_rfc3339(),
        window_to: range.to.to_rfc3339(),
        export: ExportView::new(&["mcp-servers", "mcp-calls", "mcp-tools"], &preserved_query),
        kpis,
        sort_headers: sort_headers(
            BASE_URL,
            &columns::columns(),
            &sort_key,
            sort_dir,
            &preserved_query,
        ),
        servers_count: servers.len(),
        unconfigured_count,
        builtin_calls: fleet.builtin_calls,
        servers,
        access_control_url: "/admin/access-control?entity_type=mcp_server",
        sort_key,
        sort_dir: sort_dir.to_owned(),
        search,
        preserved_query,
    };
    Ok(render_typed_page(
        &engine,
        "catalog-mcp",
        &page,
        &user_ctx,
        &mkt_ctx,
    ))
}

pub(crate) async fn mcp_detail_page(
    shell: Page,
    State(pool): State<Arc<PgPool>>,
    Path(mcp_id): Path<McpServerId>,
    Query(query): Query<McpDetailQuery>,
) -> AdminHtmlResult<Response> {
    console_only(&shell.user)?;
    let path = shared::get_services_path()?;
    let range = window_of(query.preset, query.from, query.to);

    let catalog = super::data::load_catalog(&path, &shell.user.roles);
    let server = catalog.mcp.iter().find(|s| s.id == mcp_id);
    let rt = load_runtime(&pool, range).await;

    // Why: a server the catalog does not declare but the runtime has served is
    // a real page, because the list links to it. Only a name neither half knows
    // is a 404.
    if server.is_none() && !rt.knows(mcp_id.as_str()) {
        return Err(AdminError::NotFound("No such MCP server.".to_owned()).into());
    }

    let counts = assignment_counts_by_type(&pool, ENTITY_MCP_SERVER).await;
    let included_by = catalog
        .plugins_by_mcp
        .get(mcp_id.as_str())
        .cloned()
        .unwrap_or_default();
    let row = rows::build_row(&RowInputs {
        id: mcp_id.as_str(),
        server,
        runtime: &rt,
        plugin_count: included_by.len(),
        assignment_count: counts.get(mcp_id.as_str()).copied().unwrap_or(0),
    });

    let access = super::access::panel(
        &pool,
        &shell.user,
        (ENTITY_MCP_SERVER, mcp_id.as_str()),
        query.why.as_deref(),
    )
    .await;
    let window_query = pairs_to_query(&window_pairs(&range));
    let sections = sections::detail_sections(
        &pool,
        &mcp_id,
        range,
        query.page.unwrap_or(0).max(0),
        &window_query,
    )
    .await;

    let page = detail_page_data(DetailInputs {
        id: mcp_id,
        range,
        row,
        sections,
        server,
        included_by,
        access,
    });
    Ok(render_typed_page(
        &shell.engine,
        "catalog-mcp-detail",
        &page,
        &shell.user,
        &shell.marketplace,
    ))
}

struct DetailInputs<'a> {
    id: McpServerId,
    range: TimeRange,
    row: McpServerRow,
    sections: sections::DetailSections,
    server: Option<&'a crate::types::McpServerDetail>,
    included_by: Vec<super::view::LinkedEntity>,
    access: crate::handlers::ssr::entity_panel::EntityAccessView,
}

fn detail_page_data(input: DetailInputs<'_>) -> McpDetailData {
    let DetailInputs {
        id: mcp_id,
        range,
        row,
        sections,
        server,
        included_by,
        access,
    } = input;
    let mut export_pairs = vec![("server", Some(mcp_id.as_str().to_owned()))];
    export_pairs.extend(window_pairs(&range));
    let connected_count = sections.connections.iter().filter(|c| c.connected).count();

    McpDetailData {
        page: "mcp",
        title: mcp_id.as_str().to_owned(),
        subtitle: row.description.clone(),
        breadcrumbs: vec![
            BreadcrumbView::link("MCP servers", BASE_URL),
            BreadcrumbView::current(mcp_id.as_str()),
        ],
        configured: row.configured,
        enabled: row.enabled,
        status_label: row.status_label,
        status_tone: row.status_tone,
        window_label: window_label(&range),
        // Why: an empty base keeps the picker's links relative, so they stay
        // on this server's page rather than naming a path the id must escape.
        time_range: TimeRangeContext {
            base_url: "",
            ..time_range_context(&range, BASE_URL)
        },
        export: ExportView::new(&["mcp-calls", "mcp-tools"], &pairs_to_query(&export_pairs)),
        kpis: columns::kpis(std::slice::from_ref(&row)),
        tools_count: sections.tools.len(),
        tools: sections.tools,
        callers_count: sections.callers.len(),
        callers: sections.callers,
        executions_count: sections.executions_total,
        pagination: sections.pagination,
        executions: sections.executions,
        connections_count: sections.connections.len(),
        connected_count,
        connections: sections.connections,
        access,
        config_facts: detail::config_facts(server),
        oauth_scopes: server.map(|s| s.oauth_scopes.clone()).unwrap_or_default(),
        included_by_count: included_by.len(),
        included_by,
        access_url: row.access_url.clone(),
        id: mcp_id,
    }
}
