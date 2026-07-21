//! View-model assembly for the catalog pages.
//!
//! Turns a loaded [`Catalog`] plus per-entity assignment counts into the
//! `Serialize` structs the templates consume. Loading lives in [`super::data`].

use std::collections::HashMap;

use systemprompt::identifiers::SkillId;

use crate::types::{ConfiguredHook, ENTITY_MCP_SERVER, ENTITY_PLUGIN, ENTITY_SKILL};

use super::data::Catalog;
use super::view::{
    HookRef, LinkedEntity, McpDetailData, McpListRow, PluginDetailData, PluginListRow,
    SkillDetailData, SkillListRow, matrix_url, mcp_url, plugin_url, skill_url,
};

pub(super) fn plugin_rows(catalog: Catalog, counts: &HashMap<String, i64>) -> Vec<PluginListRow> {
    catalog
        .plugins
        .into_iter()
        .map(|p| PluginListRow {
            detail_url: plugin_url(&p.id),
            matrix_url: matrix_url(ENTITY_PLUGIN, &p.id),
            skills_count: p.skills.len(),
            mcp_count: p.mcp_servers.len(),
            agents_count: p.agents.len(),
            assignment_count: counts.get(&p.id).copied().unwrap_or(0),
            id: p.id,
            name: p.name,
            description: p.description,
            category: p.category,
            version: p.version,
            enabled: p.enabled,
            source_path: p.source_path,
        })
        .collect()
}

pub(super) fn plugin_detail(
    catalog: &Catalog,
    plugin_id: &str,
    assignment_count: i64,
) -> Option<PluginDetailData> {
    let skill_names: HashMap<String, String> = catalog
        .skills
        .iter()
        .map(|s| (s.id.as_str().to_owned(), s.name.clone()))
        .collect();
    let plugin = catalog.plugins.iter().find(|p| p.id == plugin_id)?;

    let skills = plugin
        .skills
        .iter()
        .map(|s| {
            let id = s.as_str().to_owned();
            LinkedEntity {
                name: skill_names.get(&id).cloned().unwrap_or_else(|| id.clone()),
                url: skill_url(&id),
                id,
            }
        })
        .collect::<Vec<_>>();
    let mcp_servers = plugin
        .mcp_servers
        .iter()
        .map(|m| {
            let id = m.as_str().to_owned();
            LinkedEntity {
                name: id.clone(),
                url: mcp_url(&id),
                id,
            }
        })
        .collect::<Vec<_>>();
    let agents = plugin
        .agents
        .iter()
        .map(|a| {
            let id = a.as_str().to_owned();
            LinkedEntity {
                name: catalog
                    .agent_names
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| id.clone()),
                url: String::new(),
                id,
            }
        })
        .collect::<Vec<_>>();
    let hooks: Vec<HookRef> = catalog
        .hooks_by_plugin
        .get(&plugin.id)
        .map(|hs| hs.iter().map(hook_ref).collect())
        .unwrap_or_default();

    Some(PluginDetailData {
        page: "plugin-detail",
        title: plugin.name.clone(),
        matrix_url: matrix_url(ENTITY_PLUGIN, &plugin.id),
        assignment_count,
        skills_count: skills.len(),
        mcp_count: mcp_servers.len(),
        agents_count: agents.len(),
        hooks_count: hooks.len(),
        skills,
        mcp_servers,
        agents,
        hooks,
        id: plugin.id.clone(),
        name: plugin.name.clone(),
        description: plugin.description.clone(),
        version: plugin.version.clone(),
        category: plugin.category.clone(),
        enabled: plugin.enabled,
        author_name: plugin.author_name.clone(),
        keywords: plugin.keywords.clone(),
        roles: plugin.roles.clone(),
        source_path: plugin.source_path.clone(),
    })
}

fn hook_ref(h: &ConfiguredHook) -> HookRef {
    HookRef {
        id: h.id.clone(),
        event: h.event.clone(),
        matcher: h.matcher.clone(),
        command: h.command.clone(),
        is_async: h.is_async,
    }
}

pub(super) fn skill_rows(catalog: &Catalog, counts: &HashMap<String, i64>) -> Vec<SkillListRow> {
    catalog
        .skills
        .iter()
        .map(|s| {
            let id = s.id.as_str().to_owned();
            SkillListRow {
                detail_url: skill_url(&id),
                matrix_url: matrix_url(ENTITY_SKILL, &id),
                assignment_count: counts.get(&id).copied().unwrap_or(0),
                plugin_count: catalog.plugins_by_skill.get(&id).map_or(0, Vec::len),
                id,
                name: s.name.clone(),
                description: s.description.clone(),
                enabled: s.enabled,
                source_path: s.source_path.clone(),
            }
        })
        .collect()
}

pub(super) fn skill_detail(
    catalog: &Catalog,
    skill: &SkillId,
    assignment_count: i64,
) -> Option<SkillDetailData> {
    let id = skill.as_str();
    let entry = catalog.skills.iter().find(|s| s.id == *skill)?;
    let included_by = catalog
        .plugins_by_skill
        .get(id)
        .cloned()
        .unwrap_or_default();
    Some(SkillDetailData {
        page: "skill-detail",
        title: entry.name.clone(),
        matrix_url: matrix_url(ENTITY_SKILL, id),
        assignment_count,
        included_by_count: included_by.len(),
        included_by,
        id: id.to_owned(),
        name: entry.name.clone(),
        description: entry.description.clone(),
        enabled: entry.enabled,
        source_path: entry.source_path.clone(),
    })
}

pub(super) fn mcp_rows(catalog: &Catalog, counts: &HashMap<String, i64>) -> Vec<McpListRow> {
    catalog
        .mcp
        .iter()
        .map(|m| {
            let id = m.id.as_str().to_owned();
            McpListRow {
                detail_url: mcp_url(&id),
                matrix_url: matrix_url(ENTITY_MCP_SERVER, &id),
                assignment_count: counts.get(&id).copied().unwrap_or(0),
                plugin_count: catalog.plugins_by_mcp.get(&id).map_or(0, Vec::len),
                name: id.clone(),
                id,
                description: m.description.clone(),
                enabled: m.enabled,
                oauth_required: m.oauth_required,
                source_path: m.source_path.clone(),
            }
        })
        .collect()
}

pub(super) fn mcp_detail(
    catalog: &Catalog,
    mcp_id: &str,
    assignment_count: i64,
) -> Option<McpDetailData> {
    let server = catalog.mcp.iter().find(|m| m.id.as_str() == mcp_id)?;
    let included_by = catalog
        .plugins_by_mcp
        .get(mcp_id)
        .cloned()
        .unwrap_or_default();
    Some(McpDetailData {
        page: "mcp-detail",
        title: mcp_id.to_owned(),
        matrix_url: matrix_url(ENTITY_MCP_SERVER, mcp_id),
        assignment_count,
        included_by_count: included_by.len(),
        included_by,
        description: server.description.clone(),
        enabled: server.enabled,
        server_type: server.server_type.clone(),
        endpoint: server.endpoint.clone(),
        port: server.port,
        oauth_required: server.oauth_required,
        oauth_scopes: server.oauth_scopes.clone(),
        oauth_audience: server.oauth_audience.clone(),
        source_path: server.source_path.clone(),
        id: mcp_id.to_owned(),
    })
}
