//! View-model types and assembly helpers for the catalog pages.
//!
//! Three entity families — plugins, skills, MCP servers — each get a list page
//! and a detail page. Plugins are collections that reference skills, MCP
//! servers, agents, and hooks; the detail pages surface those links in both
//! directions (a plugin lists its members; a skill/server lists the plugins
//! that include it). The handlers own the request flow and data loading; this
//! module owns the row/page shaping and the cross-link URL construction.

use std::collections::HashMap;

use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use serde::Serialize;
use sqlx::PgPool;

#[derive(Debug, Clone, Serialize)]
pub(super) struct LinkedEntity {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) url: String,
}

#[derive(Debug, Serialize)]
pub(super) struct HookRef {
    pub(super) id: String,
    pub(super) event: String,
    pub(super) matcher: String,
    pub(super) command: String,
    pub(super) is_async: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct PluginListRow {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) description: String,
    pub(super) category: String,
    pub(super) version: String,
    pub(super) enabled: bool,
    pub(super) skills_count: usize,
    pub(super) mcp_count: usize,
    pub(super) agents_count: usize,
    pub(super) assignment_count: i64,
    pub(super) source_path: String,
    pub(super) detail_url: String,
    pub(super) matrix_url: String,
}

#[derive(Debug, Serialize)]
pub(super) struct PluginsPageData {
    pub(super) page: &'static str,
    pub(super) title: &'static str,
    pub(super) plugins: Vec<PluginListRow>,
    pub(super) plugins_count: usize,
}

#[derive(Debug, Serialize)]
pub(super) struct SkillListRow {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) description: String,
    pub(super) enabled: bool,
    pub(super) plugin_count: usize,
    pub(super) assignment_count: i64,
    pub(super) source_path: String,
    pub(super) detail_url: String,
    pub(super) matrix_url: String,
}

#[derive(Debug, Serialize)]
pub(super) struct SkillsPageData {
    pub(super) page: &'static str,
    pub(super) title: &'static str,
    pub(super) skills: Vec<SkillListRow>,
    pub(super) skills_count: usize,
}

#[derive(Debug, Serialize)]
pub(super) struct McpListRow {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) description: String,
    pub(super) enabled: bool,
    pub(super) oauth_required: bool,
    pub(super) plugin_count: usize,
    pub(super) assignment_count: i64,
    pub(super) source_path: String,
    pub(super) detail_url: String,
    pub(super) matrix_url: String,
}

#[derive(Debug, Serialize)]
pub(super) struct McpPageData {
    pub(super) page: &'static str,
    pub(super) title: &'static str,
    pub(super) servers: Vec<McpListRow>,
    pub(super) servers_count: usize,
}

#[derive(Debug, Serialize)]
pub(super) struct PluginDetailData {
    pub(super) page: &'static str,
    pub(super) title: String,
    pub(super) id: String,
    pub(super) name: String,
    pub(super) description: String,
    pub(super) version: String,
    pub(super) category: String,
    pub(super) enabled: bool,
    pub(super) author_name: String,
    pub(super) keywords: Vec<String>,
    pub(super) roles: Vec<String>,
    pub(super) source_path: String,
    pub(super) matrix_url: String,
    pub(super) assignment_count: i64,
    pub(super) skills: Vec<LinkedEntity>,
    pub(super) mcp_servers: Vec<LinkedEntity>,
    pub(super) agents: Vec<LinkedEntity>,
    pub(super) hooks: Vec<HookRef>,
    pub(super) skills_count: usize,
    pub(super) mcp_count: usize,
    pub(super) agents_count: usize,
    pub(super) hooks_count: usize,
}

#[derive(Debug, Serialize)]
pub(super) struct SkillDetailData {
    pub(super) page: &'static str,
    pub(super) title: String,
    pub(super) id: String,
    pub(super) name: String,
    pub(super) description: String,
    pub(super) enabled: bool,
    pub(super) source_path: String,
    pub(super) matrix_url: String,
    pub(super) assignment_count: i64,
    pub(super) included_by: Vec<LinkedEntity>,
    pub(super) included_by_count: usize,
}

#[derive(Debug, Serialize)]
pub(super) struct McpDetailData {
    pub(super) page: &'static str,
    pub(super) title: String,
    pub(super) id: String,
    pub(super) description: String,
    pub(super) enabled: bool,
    pub(super) server_type: String,
    pub(super) endpoint: String,
    pub(super) port: u16,
    pub(super) oauth_required: bool,
    pub(super) oauth_scopes: Vec<String>,
    pub(super) oauth_audience: String,
    pub(super) source_path: String,
    pub(super) matrix_url: String,
    pub(super) assignment_count: i64,
    pub(super) included_by: Vec<LinkedEntity>,
    pub(super) included_by_count: usize,
}

pub(super) fn matrix_url(entity_type: &str, entity_id: &str) -> String {
    format!("/admin/access/matrix?entity_type={entity_type}&entity_id={entity_id}")
}

pub(super) fn plugin_url(id: &str) -> String {
    format!("/admin/catalog/plugins/{id}")
}

pub(super) fn skill_url(id: &str) -> String {
    format!("/admin/catalog/skills/{id}")
}

pub(super) fn mcp_url(id: &str) -> String {
    format!("/admin/catalog/mcp/{id}")
}

pub(super) async fn assignment_counts_by_type(
    pool: &PgPool,
    entity_type: &str,
) -> HashMap<String, i64> {
    crate::repositories::users::access_control::count_assignments_by_entity_type(pool, entity_type)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, entity_type, "Failed to load assignment counts");
            HashMap::new()
        })
}

pub(super) fn forbidden() -> Response {
    (
        StatusCode::FORBIDDEN,
        Html("<h1>Access Denied</h1><p>Admin access required.</p>"),
    )
        .into_response()
}

pub(super) fn not_found(kind: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Html(format!("<h1>Not found</h1><p>No such {kind}.</p>")),
    )
        .into_response()
}
