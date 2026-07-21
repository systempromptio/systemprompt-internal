//! Read-only catalog admin pages for the three installable entity families:
//!
//! - `/admin/catalog/plugins` — plugins (collections) from
//!   `services/plugins/*/config.yaml`, each referencing skills, MCP servers,
//!   agents, and hooks.
//! - `/admin/catalog/skills` — skills from `services/skills/*`.
//! - `/admin/catalog/mcp` — MCP servers from `services/mcp/*`.
//!
//! Each family has a list page and a detail page. Detail pages surface the
//! plugin ↔ member relationship in both directions. All pages are strictly
//! read-only: operators edit `services/*.yaml` and restart.

mod data;
mod view;
mod view_models;

use std::sync::Arc;

use axum::extract::{Extension, Path, State};
use axum::response::Response;
use sqlx::PgPool;

use crate::handlers::shared;
use crate::templates::AdminTemplateEngine;
use crate::types::{
    ENTITY_MCP_SERVER, ENTITY_PLUGIN, ENTITY_SKILL, MarketplaceContext, UserContext,
};

use super::ssr::ssr_helpers::render_typed_page;
use view::{assignment_counts_by_type, forbidden, not_found};

pub(crate) async fn plugins_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
) -> Response {
    if !user_ctx.is_admin {
        return forbidden();
    }
    let path = match shared::get_services_path() {
        Ok(p) => p,
        Err(r) => return *r,
    };

    let catalog = data::load_catalog(&path, &user_ctx.roles);
    let counts = assignment_counts_by_type(&pool, ENTITY_PLUGIN).await;
    let plugins = view_models::plugin_rows(catalog, &counts);
    let page = view::PluginsPageData {
        page: "plugins",
        title: "Plugins",
        plugins_count: plugins.len(),
        plugins,
    };
    render_typed_page(&engine, "catalog-plugins", &page, &user_ctx, &mkt_ctx)
}

pub(crate) async fn plugin_detail_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
    Path(plugin_id): Path<String>,
) -> Response {
    if !user_ctx.is_admin {
        return forbidden();
    }
    let path = match shared::get_services_path() {
        Ok(p) => p,
        Err(r) => return *r,
    };

    let catalog = data::load_catalog(&path, &user_ctx.roles);
    let counts = assignment_counts_by_type(&pool, ENTITY_PLUGIN).await;
    let assignment_count = counts.get(&plugin_id).copied().unwrap_or(0);
    view_models::plugin_detail(&catalog, &plugin_id, assignment_count).map_or_else(
        || not_found("plugin"),
        |page| render_typed_page(&engine, "catalog-plugin-detail", &page, &user_ctx, &mkt_ctx),
    )
}

pub(crate) async fn skills_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
) -> Response {
    if !user_ctx.is_admin {
        return forbidden();
    }
    let path = match shared::get_services_path() {
        Ok(p) => p,
        Err(r) => return *r,
    };

    let catalog = data::load_catalog(&path, &user_ctx.roles);
    let counts = assignment_counts_by_type(&pool, ENTITY_SKILL).await;
    let skills = view_models::skill_rows(&catalog, &counts);
    let page = view::SkillsPageData {
        page: "skills",
        title: "Skills",
        skills_count: skills.len(),
        skills,
    };
    render_typed_page(&engine, "catalog-skills", &page, &user_ctx, &mkt_ctx)
}

pub(crate) async fn skill_detail_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
    Path(skill_id): Path<String>,
) -> Response {
    if !user_ctx.is_admin {
        return forbidden();
    }
    let path = match shared::get_services_path() {
        Ok(p) => p,
        Err(r) => return *r,
    };

    let catalog = data::load_catalog(&path, &user_ctx.roles);
    let counts = assignment_counts_by_type(&pool, ENTITY_SKILL).await;
    let assignment_count = counts.get(&skill_id).copied().unwrap_or(0);
    let skill = systemprompt::identifiers::SkillId::new(&skill_id);
    view_models::skill_detail(&catalog, &skill, assignment_count).map_or_else(
        || not_found("skill"),
        |page| render_typed_page(&engine, "catalog-skill-detail", &page, &user_ctx, &mkt_ctx),
    )
}

pub(crate) async fn mcp_servers_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
) -> Response {
    if !user_ctx.is_admin {
        return forbidden();
    }
    let path = match shared::get_services_path() {
        Ok(p) => p,
        Err(r) => return *r,
    };

    let catalog = data::load_catalog(&path, &user_ctx.roles);
    let counts = assignment_counts_by_type(&pool, ENTITY_MCP_SERVER).await;
    let servers = view_models::mcp_rows(&catalog, &counts);
    let page = view::McpPageData {
        page: "mcp",
        title: "MCP servers",
        servers_count: servers.len(),
        servers,
    };
    render_typed_page(&engine, "catalog-mcp", &page, &user_ctx, &mkt_ctx)
}

pub(crate) async fn mcp_detail_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
    Path(mcp_id): Path<String>,
) -> Response {
    if !user_ctx.is_admin {
        return forbidden();
    }
    let path = match shared::get_services_path() {
        Ok(p) => p,
        Err(r) => return *r,
    };

    let catalog = data::load_catalog(&path, &user_ctx.roles);
    let counts = assignment_counts_by_type(&pool, ENTITY_MCP_SERVER).await;
    let assignment_count = counts.get(&mcp_id).copied().unwrap_or(0);
    view_models::mcp_detail(&catalog, &mcp_id, assignment_count).map_or_else(
        || not_found("MCP server"),
        |page| render_typed_page(&engine, "catalog-mcp-detail", &page, &user_ctx, &mkt_ctx),
    )
}
