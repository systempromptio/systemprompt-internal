//! Odoo group → platform role mapping for federated sign-in.
//!
//! Odoo is this deployment's identity provider, so it is also the role
//! authority: on every Odoo sign-in the login handler asks
//! [`resolve_roles`] for the platform roles the just-authenticated user
//! should hold, computed from their Odoo groups through
//! `services/access-control/odoo-roles.yaml`. `None` means the lookup could
//! not complete — the caller keeps the user's existing roles rather than
//! guessing, which is the safe direction in both ways: a failed lookup never
//! grants admin and never strips roles it could not recompute.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;

use super::rpc::{OdooConnection, OdooUserSession, user_group_xml_ids};

const MAPPING_FILE: &str = "access-control/odoo-roles.yaml";

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OdooRoleMap {
    #[serde(default)]
    pub default_roles: Vec<String>,
    #[serde(default)]
    pub groups: std::collections::BTreeMap<String, Vec<String>>,
}

impl OdooRoleMap {
    #[must_use]
    pub fn roles_for(&self, group_xml_ids: &[String]) -> Vec<String> {
        let mut roles: BTreeSet<&str> = self.default_roles.iter().map(String::as_str).collect();
        for xml_id in group_xml_ids {
            if let Some(granted) = self.groups.get(xml_id) {
                roles.extend(granted.iter().map(String::as_str));
            }
        }
        roles.into_iter().map(str::to_owned).collect()
    }
}

fn load_mapping(services_path: &Path) -> Option<OdooRoleMap> {
    let path = services_path.join(MAPPING_FILE);
    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "Odoo role mapping unreadable");
            return None;
        },
    };
    match serde_yaml::from_str(&raw) {
        Ok(mapping) => Some(mapping),
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "Odoo role mapping invalid");
            None
        },
    }
}

pub(crate) async fn resolve_roles(
    conn: &OdooConnection,
    uid: i32,
    credential: &str,
) -> Option<Vec<String>> {
    let services_path = systemprompt::models::Config::get()
        .ok()
        .map(|c| std::path::PathBuf::from(&c.services_path))?;
    let mapping = load_mapping(&services_path)?;
    let session = OdooUserSession {
        conn,
        uid,
        credential,
    };
    match user_group_xml_ids(&session).await {
        Ok(xml_ids) => Some(mapping.roles_for(&xml_ids)),
        Err(e) => {
            tracing::warn!(
                error = %e,
                odoo_uid = uid,
                "Odoo group lookup failed; keeping the user's existing platform roles"
            );
            None
        },
    }
}
