//! Catalog data loading and view-model assembly.
//!
//! `load_catalog` reads the plugins, skills, MCP servers, agents, and hooks
//! from `services/` once and derives the reverse indexes that let a skill or
//! MCP server list the plugins including it. The `*_rows` / `*_detail` builders
//! turn that snapshot plus per-entity assignment counts into the `Serialize`
//! page structs the templates consume. Handlers own request flow; this module
//! owns the shaping.

use std::collections::HashMap;

use systemprompt::identifiers::SkillId;

use crate::repositories;
use crate::types::{ConfiguredHook, McpServerDetail, PluginDetail, SkillCatalogEntry};

use super::view::{
    EntityRef, HookRef, McpDetailData, McpListRow, PluginDetailData, PluginListRow,
    SkillDetailData, SkillListRow, matrix_url, mcp_url, plugin_url, skill_url,
};
use crate::types::{ENTITY_MCP_SERVER, ENTITY_PLUGIN, ENTITY_SKILL};

pub(super) struct Catalog {
    pub(super) plugins: Vec<PluginDetail>,
    pub(super) skills: Vec<SkillCatalogEntry>,
    pub(super) mcp: Vec<McpServerDetail>,
    pub(super) agent_names: HashMap<String, String>,
    pub(super) hooks_by_plugin: HashMap<String, Vec<ConfiguredHook>>,
    pub(super) plugins_by_skill: HashMap<String, Vec<EntityRef>>,
    pub(super) plugins_by_mcp: HashMap<String, Vec<EntityRef>>,
}

fn to_entity_refs(
    map: repositories::marketplace::plugin_maps::EntityPluginMap,
) -> HashMap<String, Vec<EntityRef>> {
    map.into_iter()
        .map(|(entity_id, plugins)| {
            let refs = plugins
                .into_iter()
                .map(|p| EntityRef {
                    id: p.0.clone(),
                    name: p.1.clone(),
                    url: plugin_url(&p.0),
                })
                .collect();
            (entity_id, refs)
        })
        .collect()
}

pub(super) fn load_catalog(services_path: &std::path::Path, roles: &[String]) -> Catalog {
    let plugins = repositories::marketplace::plugins::list_plugin_catalog(services_path)
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "Failed to load plugin catalog");
            Vec::new()
        });
    let skills = repositories::marketplace::plugins::list_skill_catalog(services_path)
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "Failed to load skill catalog");
            Vec::new()
        });
    let mcp = repositories::mcp::mcp_servers::list_mcp_servers(services_path).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "Failed to load MCP catalog");
        Vec::new()
    });
    let agent_names = repositories::marketplace::plugins::list_agent_catalog(services_path)
        .unwrap_or_default()
        .into_iter()
        .map(|a| (a.id.as_str().to_owned(), a.name))
        .collect();
    let hooks = repositories::marketplace::hooks::list_configured_hooks(services_path, roles)
        .unwrap_or_default();

    let mut hooks_by_plugin: HashMap<String, Vec<ConfiguredHook>> = HashMap::new();
    for hook in hooks {
        hooks_by_plugin
            .entry(hook.plugin_id.as_str().to_owned())
            .or_default()
            .push(hook);
    }

    let (skill_map, _agent_map, mcp_map) =
        repositories::marketplace::plugin_maps::build_entity_plugin_maps(services_path);

    Catalog {
        plugins,
        skills,
        mcp,
        agent_names,
        hooks_by_plugin,
        plugins_by_skill: to_entity_refs(skill_map),
        plugins_by_mcp: to_entity_refs(mcp_map),
    }
}

