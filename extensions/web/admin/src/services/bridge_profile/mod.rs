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
    build_agents_block, build_bridge_profile_block, build_usage, load_usage_sections,
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
    // Why: the store this identity was resolved from — the same one the usage
    // query is keyed on, by construction, so the banner cannot claim an
    // identity the data did not come from.
    pub source: crate::types::IdentitySource,
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
    pub install_command: String,
    pub login_command: String,
    pub just_install_command: String,
    pub just_login_command: String,
}

pub(crate) use systemprompt_internal_brand::BRIDGE_BINARY_NAME as BRIDGE_BINARY;

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct OdooLinkBlock {
    pub linked: bool,
    pub odoo_login: Option<String>,
    pub configured: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct BridgeProfilePageData {
    pub page: &'static str,
    pub title: &'static str,
    pub identity: ProfileIdentity,
    pub bridge_connect: Option<BridgeConnectBlock>,
    pub bridge_profile: Option<BridgeProfileBlock>,
    pub usage: systemprompt::models::api::cloud::BridgeProfileUsage,
    pub agents: AgentsBlock,
    pub odoo: OdooLinkBlock,
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
        install_command: crate::services::bridge_downloads::install_command(
            &gateway,
            Some(&issued.code),
        ),
        login_command: format!(
            "{BRIDGE_BINARY} login --code {code} --gateway {gateway}",
            code = issued.code
        ),
        just_install_command: format!("just claude {code} {gateway}", code = issued.code),
        just_login_command: format!("just connect {code} {gateway}", code = issued.code),
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

    let sections = load_usage_sections(&pool, &user_id).await;
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
        source: user_ctx.source,
    };

    let usage = build_usage(sections);
    let agents = build_agents_block();
    let odoo = build_odoo_block(&pool, &user_id).await;

    BridgeProfilePageData {
        page: "profile",
        title: "Profile",
        identity,
        bridge_connect,
        bridge_profile,
        usage,
        agents,
        odoo,
    }
}

// Why: the card shows *which* Odoo account is connected, because a user with
// access to two Odoo logins needs to know which one their agents are acting as
// — that is what Odoo's audit log will show against the records they change.
// The stored API key is never read here.
async fn build_odoo_block(pool: &PgPool, user_id: &UserId) -> OdooLinkBlock {
    use crate::handlers::odoo_auth::OdooConnection;
    use crate::repositories::users::odoo_identity;

    let identity = odoo_identity::find(pool, user_id)
        .await
        .map_err(|e| tracing::warn!(error = %e, "could not read Odoo link status"))
        .ok()
        .flatten();

    OdooLinkBlock {
        linked: identity.is_some(),
        odoo_login: identity.map(|i| i.odoo_login),
        configured: OdooConnection::from_env().is_some(),
    }
}
