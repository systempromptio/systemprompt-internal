//! The plugin and skill detail pages: the catalog entry, its relationships
//! in both directions, and the shared "Who gets this" panel.

use std::sync::Arc;
use systemprompt::identifiers::PluginId;

use axum::extract::{Path, Query, State};
use axum::response::Response;
use sqlx::PgPool;

use super::super::ssr::ssr_helpers::render_typed_page;
use super::view::{PanelQuery, assignment_counts_by_type};
use super::{access, admin_only, data, view_models};
use crate::error::{AdminError, AdminHtmlResult};
use crate::handlers::shared;
use crate::handlers::ssr::page::Page;
use crate::types::{ENTITY_PLUGIN, ENTITY_SKILL};

pub(crate) async fn plugin_detail_page(
    shell: Page,
    State(pool): State<Arc<PgPool>>,
    Path(plugin_id): Path<PluginId>,
    Query(query): Query<PanelQuery>,
) -> AdminHtmlResult<Response> {
    let Page {
        engine,
        user: user_ctx,
        marketplace: mkt_ctx,
    } = shell;
    admin_only(&user_ctx)?;
    let path = shared::get_services_path()?;

    let catalog = data::load_catalog(&path, &user_ctx.roles);
    let counts = assignment_counts_by_type(&pool, ENTITY_PLUGIN).await;
    let assignment_count = counts.get(plugin_id.as_str()).copied().unwrap_or(0);
    let mut page = view_models::plugin_detail(&catalog, &plugin_id, assignment_count)
        .ok_or_else(|| AdminError::NotFound("No such plugin.".to_owned()))?;
    page.access = Some(
        access::panel(
            &pool,
            &user_ctx,
            (ENTITY_PLUGIN, plugin_id.as_str()),
            query.why.as_deref(),
        )
        .await,
    );
    Ok(render_typed_page(
        &engine,
        "catalog-plugin-detail",
        &page,
        &user_ctx,
        &mkt_ctx,
    ))
}

pub(crate) async fn skill_detail_page(
    shell: Page,
    State(pool): State<Arc<PgPool>>,
    Path(skill_id): Path<String>,
    Query(query): Query<PanelQuery>,
) -> AdminHtmlResult<Response> {
    let Page {
        engine,
        user: user_ctx,
        marketplace: mkt_ctx,
    } = shell;
    admin_only(&user_ctx)?;
    let path = shared::get_services_path()?;

    let catalog = data::load_catalog(&path, &user_ctx.roles);
    let counts = assignment_counts_by_type(&pool, ENTITY_SKILL).await;
    let assignment_count = counts.get(&skill_id).copied().unwrap_or(0);
    let skill = systemprompt::identifiers::SkillId::new(&skill_id);
    let mut page = view_models::skill_detail(&catalog, &skill, assignment_count)
        .ok_or_else(|| AdminError::NotFound("No such skill.".to_owned()))?;
    page.access = Some(
        access::panel(
            &pool,
            &user_ctx,
            (ENTITY_SKILL, &skill_id),
            query.why.as_deref(),
        )
        .await,
    );
    Ok(render_typed_page(
        &engine,
        "catalog-skill-detail",
        &page,
        &user_ctx,
        &mkt_ctx,
    ))
}
