//! The `comms_whoami` report: who the caller is, what they are linked to, and
//! exactly what their role grants.
//!
//! Grants are resolved with core's parent-chain resolver over the same
//! `access_control_rules` the bridge manifest is filtered with, so the panel
//! can never disagree with what the bridge installed. The Odoo link is
//! reported by login and uid only; the API key never leaves `odoo_identity`.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt::identifiers::UserId;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WhoamiReport {
    pub user: WhoamiUser,
    pub odoo: OdooLinkStatus,
    pub grants: WhoamiGrants,
    pub sessions: Vec<OwnSession>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WhoamiUser {
    pub id: UserId,
    pub email: String,
    pub display_name: Option<String>,
    pub roles: Vec<String>,
    pub department: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OdooLinkStatus {
    pub linked: bool,
    pub login: Option<String>,
    pub uid: Option<i32>,
    pub linked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct WhoamiGrants {
    pub marketplaces: Vec<GrantedEntity>,
    pub plugins: Vec<GrantedEntity>,
    pub mcp_servers: Vec<GrantedEntity>,
    pub skills: Vec<GrantedEntity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GrantedEntity {
    pub id: String,
    pub via: GrantSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "kind", content = "id")]
pub enum GrantSource {
    Own,
    Plugin(String),
    Marketplace(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OwnSession {
    pub handle: String,
    pub workspace: Option<String>,
    pub git_branch: Option<String>,
    pub current_activity: Option<String>,
    pub last_event_at: Option<DateTime<Utc>>,
}
