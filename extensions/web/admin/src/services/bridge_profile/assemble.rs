//! Per-section assembly for the bridge profile payload.
//!
//! Each function owns one card on the profile pane: the concurrent usage
//! fan-out, the usage view-model, config/identity strings, the bridge gateway
//! block, and the agents block. Falls back to empty defaults on failure so a
//! missing section renders as an empty card rather than a page-level error.

use std::path::PathBuf;
use std::sync::Arc;

use sqlx::PgPool;
use systemprompt::config::ProfileBootstrap;
use systemprompt::identifiers::{TenantId, UserId};
use systemprompt::models::Config;
use uuid::Uuid;

use systemprompt::analytics::ProfileUsageService;
use systemprompt::models::api::cloud::BridgeProfileUsage;

use crate::repositories::bridge::{BridgeUserRow, find_bridge_user};

use super::{AgentItem, AgentsBlock, BridgeProfileBlock};

pub(super) struct UsageSections {
    pub(super) usage: BridgeProfileUsage,
    pub(super) bridge_user: Option<BridgeUserRow>,
}

// Why: the windows come from core's `ProfileUsageService` — the same derivation
// `/v1/bridge/profile/usage` serves. This page used to run its own SQL, so the
// two surfaces could and did report different numbers for one metric.
pub(super) async fn load_usage_sections(pool: &Arc<PgPool>, user_id: &UserId) -> UsageSections {
    let usage_service = ProfileUsageService::from_pool(Arc::clone(pool));
    let pool_for_user = Arc::clone(pool);
    let user_id_usage = user_id.to_owned();
    let user_id_user = user_id.to_owned();

    let (usage, bridge_user) = tokio::join!(
        async move {
            usage_service
                .get_profile_usage(&user_id_usage, chrono::Utc::now())
                .await
                .inspect_err(|e| {
                    tracing::warn!(error = %e, user_id = %user_id_usage, "bridge_profile: profile usage failed");
                })
                .unwrap_or_default()
        },
        async move {
            find_bridge_user(&pool_for_user, &user_id_user)
                .await
                .inspect_err(|e| {
                    tracing::warn!(error = %e, user_id = %user_id_user, "bridge_profile: find_bridge_user failed");
                })
                .ok()
                .flatten()
        }
    );

    UsageSections { usage, bridge_user }
}

pub(super) fn build_usage(sections: UsageSections) -> BridgeProfileUsage {
    sections.usage
}

pub(super) fn read_config_strings() -> (Option<String>, Option<String>) {
    Config::get().map_or((None, None), |c| {
        (
            Some(c.jwt_issuer.clone()),
            Some(c.api_external_url.trim_end_matches('/').to_owned()),
        )
    })
}

pub(super) fn read_tenant_id() -> Option<TenantId> {
    let bootstrap = ProfileBootstrap::get().ok()?;
    bootstrap
        .cloud
        .as_ref()
        .and_then(|cloud| cloud.tenant_id.clone())
}

pub(super) fn build_bridge_profile_block() -> Option<BridgeProfileBlock> {
    let profile = ProfileBootstrap::get().ok()?;
    let gateway = profile
        .gateway
        .as_ref()
        .and_then(systemprompt::models::profile::GatewayState::resolved)
        .filter(|g| g.enabled)?;

    let base = profile.server.api_external_url.trim_end_matches('/');
    let prefix = gateway.inference_path_prefix.trim_end_matches('/');
    let inference_gateway_base_url = format!("{base}{prefix}");

    let models: Vec<String> = profile
        .providers
        .advertised_model_ids(&[])
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    let organization_uuid = profile
        .cloud
        .as_ref()
        .and_then(|cloud| cloud.tenant_id.as_ref())
        .map(canonicalize_org_uuid);

    let models_count = models.len();
    Some(BridgeProfileBlock {
        inference_gateway_base_url,
        auth_scheme: gateway.auth_scheme.clone(),
        models,
        models_count,
        organization_uuid,
    })
}

fn canonicalize_org_uuid(tenant_id: &TenantId) -> String {
    let s = tenant_id.as_str();
    let suffix = s.strip_prefix("local_").unwrap_or(s);
    if let Ok(parsed) = Uuid::parse_str(suffix) {
        return parsed.to_string();
    }
    Uuid::new_v5(&Uuid::NAMESPACE_OID, s.as_bytes()).to_string()
}

pub(super) fn build_agents_block() -> AgentsBlock {
    let services_path = match ProfileBootstrap::get() {
        Ok(p) => PathBuf::from(&p.paths.services),
        Err(_) => return AgentsBlock::default(),
    };

    let agents = match crate::repositories::config::agents::list_configured_agents(&services_path) {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!(error = %e, "list_configured_agents failed for profile pane");
            return AgentsBlock::default();
        },
    };

    let visible: Vec<_> = agents.into_iter().filter(|a| a.show_in_ui).collect();
    let total = visible.len() as i64;
    let enabled = visible.iter().filter(|a| a.enabled).count() as i64;

    let items = visible
        .into_iter()
        .map(|a| AgentItem {
            id: a.id.as_str().to_owned(),
            display_name: if a.name.is_empty() {
                a.id.as_str().to_owned()
            } else {
                a.name
            },
            enabled: a.enabled,
            host_running: false,
        })
        .collect();

    AgentsBlock {
        total,
        enabled,
        items,
    }
}
