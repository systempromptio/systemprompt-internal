//! Restricts the public skills page to the publicly advertisable skills.
//!
//! `/skills/` is prerendered by `publish_pipeline`, so there is no caller and
//! no per-user filtering is possible. The static equivalent is the role
//! boundary the rest of the system already uses: a plugin is the role boundary
//! (`services/access-control/roles.yaml`), so a skill is advertisable when its
//! owning plugin's `entity_type: plugin` rule names the `user` role. That drops
//! the whole admin control plane — which a visitor could never run — without
//! needing to know who the visitor is.
//!
//! Failing open is deliberate: if either file is missing or unparseable the
//! page renders as it did before this filter existed, because a catalogue that
//! silently loses every entry is a worse failure than one that lists too much.
//! `scripts/validate-services.sh` is what guarantees the inputs are sane.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::Deserialize;

const USER_ROLE: &str = "user";

#[derive(Debug, Deserialize)]
struct PluginFile {
    plugin: PluginBody,
}

#[derive(Debug, Deserialize)]
struct PluginBody {
    id: String,
    #[serde(default)]
    skills: MemberList,
}

#[derive(Debug, Default, Deserialize)]
struct MemberList {
    #[serde(default)]
    include: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RolesFile {
    #[serde(default)]
    rules: Vec<RoleRule>,
}

#[derive(Debug, Deserialize)]
struct RoleRule {
    entity_type: String,
    #[serde(default)]
    entity_id: Option<String>,
    #[serde(default)]
    access: Option<String>,
    #[serde(default)]
    roles: Vec<String>,
}

// Why: `None` means "scope undeterminable, do not filter" — never "filter
// everything out". See the fail-open note in the module docs.
pub(crate) fn public_skill_ids(services_dir: &Path) -> Option<HashSet<String>> {
    let user_plugins =
        user_scoped_plugins(&services_dir.join("access-control").join("roles.yaml"))?;
    let members = plugin_members(&services_dir.join("plugins"))?;

    let ids: HashSet<String> = members
        .into_iter()
        .filter(|(plugin_id, _)| user_plugins.contains(plugin_id))
        .flat_map(|(_, skills)| skills)
        .collect();

    if ids.is_empty() {
        tracing::warn!(
            "No user-scoped plugin claims any skill; leaving the skills page unfiltered"
        );
        return None;
    }
    Some(ids)
}

fn user_scoped_plugins(roles_path: &Path) -> Option<HashSet<String>> {
    let content = std::fs::read_to_string(roles_path)
        .map_err(|e| tracing::debug!(path = %roles_path.display(), error = %e, "No roles.yaml"))
        .ok()?;
    let parsed: RolesFile = serde_yaml::from_str(&content)
        .map_err(|e| tracing::warn!(error = %e, "Unparseable roles.yaml"))
        .ok()?;

    Some(
        parsed
            .rules
            .into_iter()
            .filter(|r| r.entity_type == "plugin")
            .filter(|r| r.access.as_deref().unwrap_or("allow") == "allow")
            .filter(|r| r.roles.iter().any(|role| role == USER_ROLE))
            .filter_map(|r| r.entity_id)
            .collect(),
    )
}

fn plugin_members(plugins_dir: &Path) -> Option<HashMap<String, Vec<String>>> {
    let entries = std::fs::read_dir(plugins_dir)
        .map_err(|e| tracing::debug!(path = %plugins_dir.display(), error = %e, "No plugins dir"))
        .ok()?;

    let members: HashMap<String, Vec<String>> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let path = e.path().join("config.yaml");
            let content = std::fs::read_to_string(&path).ok()?;
            match serde_yaml::from_str::<PluginFile>(&content) {
                Ok(f) => Some((f.plugin.id, f.plugin.skills.include)),
                Err(err) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %err,
                        "Skipping plugin with unparseable config.yaml"
                    );
                    None
                },
            }
        })
        .collect();

    (!members.is_empty()).then_some(members)
}
