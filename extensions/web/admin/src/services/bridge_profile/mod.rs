//! Aggregator for the bridge-style profile pane.
//!
//! Produces the same payload shape consumed by the bridge GUI's profile tab
//! so the SSR profile page and (future) `/v1/bridge/profile/usage` endpoint
//! render the same data from the same source.

mod assemble;

use std::sync::Arc;

use serde::Serialize;
use sqlx::PgPool;
use systemprompt::identifiers::{TenantId, UserId};

use crate::types::UserContext;

use assemble::{
    build_agents_block, build_bridge_profile_block, build_usage, fetch_usage_sections,
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
pub(crate) struct BridgeProfileBlock {
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
pub(crate) struct BridgeConnectBlock {
    pub code: String,
    pub expires_in_seconds: i64,
    pub gateway: String,
    /// For a machine with no bridge yet.
    pub install_command: String,
    /// For a machine that already has one.
    pub login_command: String,
}

// Why: not derivable here — `brand()` lives in the bridge crate, which the
// admin extension does not depend on.
pub(crate) const BRIDGE_BINARY: &str = "astound-bridge";

#[derive(Debug, Clone, Serialize)]
pub(crate) struct BridgeProfilePageData {
    pub page: &'static str,
    pub title: &'static str,
    pub identity: ProfileIdentity,
    pub bridge_connect: Option<BridgeConnectBlock>,
    pub bridge_profile: Option<BridgeProfileBlock>,
    pub usage: ProfileUsage,
    pub agents: AgentsBlock,
}

async fn build_bridge_connect(
    pool: &PgPool,
    user_ctx: &UserContext,
    gateway: Option<&str>,
) -> Option<BridgeConnectBlock> {
    let gateway = gateway?.to_owned();
    let issued = crate::repositories::bridge::issue_exchange_code(pool, &user_ctx.user_id)
        .await
        .map_err(|e| {
            tracing::warn!(
                error = %e,
                "could not mint a bridge exchange code for the profile page"
            );
        })
        .ok()?;

    let expires_in_seconds = (issued.expires_at - chrono::Utc::now())
        .num_seconds()
        .max(0);

    Some(BridgeConnectBlock {
        install_command: format!(
            "curl -fsSL {gateway}/files/downloads/install.sh | sh -s -- \
             --download-base {gateway}/files/downloads --code {code}",
            code = issued.code
        ),
        login_command: format!(
            "{BRIDGE_BINARY} login --code {code} --gateway {gateway}",
            code = issued.code
        ),
        code: issued.code,
        expires_in_seconds,
        gateway,
    })
}

// Why: Build the full payload. Falls back gracefully when individual sections
// fail — the bridge does the same so missing data renders as empty cards rather
// than a page-level error.
pub(crate) async fn build_bridge_profile_data(
    pool: Arc<PgPool>,
    user_ctx: &UserContext,
) -> BridgeProfilePageData {
    let user_id = user_ctx.user_id.clone();

    let sections = fetch_usage_sections(&pool, &user_id).await;
    let display_name = sections
        .bridge_user
        .as_ref()
        .and_then(|u| u.display_name.clone());

    let (jwt_issuer, gateway_url) = read_config_strings();
    let bridge_profile = build_bridge_profile_block();
    let bridge_connect = build_bridge_connect(&pool, user_ctx, gateway_url.as_deref()).await;

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

    BridgeProfilePageData {
        page: "profile",
        title: "Profile",
        identity,
        bridge_connect,
        bridge_profile,
        usage,
        agents,
    }
}
