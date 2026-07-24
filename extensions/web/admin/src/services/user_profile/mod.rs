//! Aggregator for the profile pane.
//!
//! Assembles the signed-in user's identity, gateway access, usage rollups, and
//! visible agents into the payload the SSR profile page renders.

mod assemble;

use std::sync::Arc;

use serde::Serialize;
use sqlx::PgPool;
use systemprompt::identifiers::{TenantId, UserId};

use crate::types::UserContext;

use assemble::{
    build_agents_block, build_gateway_access_block, build_usage, fetch_usage_sections,
    read_config_strings, read_tenant_id,
};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProfileIdentity {
    pub email: String,
    pub display_name: Option<String>,
    pub user_id: UserId,
    pub tenant_id: Option<TenantId>,
    pub provider: Option<String>,
    pub roles: Vec<String>,
    pub jwt_issuer: Option<String>,
    pub gateway: Option<String>,
    pub is_admin: bool,
}

pub(crate) use crate::repositories::users::usage::{ConversationSummary, ModelShare, UsageWindow};

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct ProfileUsage {
    pub d1: UsageWindow,
    pub d7: UsageWindow,
    pub d30: UsageWindow,
    pub top_models: Vec<ModelShare>,
    pub conversations: ConversationSummary,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct GatewayAccessBlock {
    pub inference_gateway_base_url: String,
    pub auth_scheme: String,
    pub models: Vec<String>,
    pub models_count: usize,
    pub organization_uuid: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AgentItem {
    pub id: String,
    pub display_name: String,
    pub enabled: bool,
    pub host_running: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct AgentsBlock {
    pub total: i64,
    pub enabled: i64,
    pub items: Vec<AgentItem>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProfilePageData {
    pub page: &'static str,
    pub title: &'static str,
    pub identity: ProfileIdentity,
    pub gateway_access: Option<GatewayAccessBlock>,
    pub usage: ProfileUsage,
    pub agents: AgentsBlock,
}

// Why: falls back gracefully when individual sections fail, so missing data
// renders as empty cards rather than a page-level error.
pub(crate) async fn build_profile_data(
    pool: Arc<PgPool>,
    user_ctx: &UserContext,
) -> ProfilePageData {
    let user_id = user_ctx.user_id.clone();

    let sections = fetch_usage_sections(&pool, &user_id).await;
    let display_name = sections
        .identity
        .as_ref()
        .and_then(|u| u.display_name.clone());

    let (jwt_issuer, gateway_url) = read_config_strings();
    let gateway_access = build_gateway_access_block();

    let identity = ProfileIdentity {
        email: user_ctx.email.as_str().to_owned(),
        display_name,
        user_id: user_ctx.user_id.clone(),
        tenant_id: read_tenant_id(),
        provider: None,
        roles: user_ctx.roles.clone(),
        jwt_issuer,
        gateway: gateway_url,
        is_admin: user_ctx.is_admin,
    };

    let usage = build_usage(sections);
    let agents = build_agents_block();

    ProfilePageData {
        page: "profile",
        title: "Profile",
        identity,
        gateway_access,
        usage,
        agents,
    }
}
