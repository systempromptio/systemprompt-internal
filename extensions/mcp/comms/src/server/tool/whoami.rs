//! The `comms_whoami` handler.
//!
//! Grants come from core's `ParentChainIndex` over the live services config
//! and `access_control_rules` — the resolver the bridge manifest is filtered
//! with — so what this reports and what the bridge mounted are one answer.
//! Subject attributes beyond `user` and `role` (a department dimension the
//! admin extension declares) are not evaluated here; a grant conferred only
//! by such a dimension shows on the admin access matrix, not in this panel.

use std::collections::{BTreeSet, HashSet};

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::{McpExecutionId, PluginId};
use systemprompt::loader::ConfigLoader;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext as SysRequestContext;
use systemprompt::models::services::ServicesConfig;
use systemprompt::security::authz::{
    AccessControlRepository, BulkKeepQuery, ChainSources, EntityKind, NO_SUBJECT_ATTRIBUTES,
    ParentChainIndex, allowed_ids,
};

use crate::store::{CommsStore, IdentityRow};
use crate::tools::{TOOL_WHOAMI, WhoamiInput};
use crate::whoami::{
    GrantSource, GrantedEntity, OdooLinkStatus, OwnSession, WhoamiGrants, WhoamiReport, WhoamiUser,
};

use super::common::{internal, text_artifact};

pub(super) struct WhoamiHandler {
    pub(super) store: CommsStore,
}

impl McpToolHandler for WhoamiHandler {
    type Input = WhoamiInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_WHOAMI
    }

    fn description(&self) -> &'static str {
        "Report the caller's identity, Odoo link, grants and live sessions."
    }

    async fn handle(
        &self,
        _input: Self::Input,
        ctx: &SysRequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(Self::Output, String), McpError> {
        let user_id = ctx.user_id().clone();
        let identity = self
            .store
            .find_identity(&user_id)
            .await
            .map_err(internal)?
            .ok_or_else(|| internal(format!("no account row for user {user_id}")))?;

        let odoo = self
            .store
            .find_odoo_link(&user_id)
            .await
            .map_err(internal)?
            .map_or(
                OdooLinkStatus {
                    linked: false,
                    login: None,
                    uid: None,
                    linked_at: None,
                },
                |link| OdooLinkStatus {
                    linked: true,
                    login: Some(link.odoo_login),
                    uid: Some(link.odoo_uid),
                    linked_at: Some(link.linked_at),
                },
            );

        let sessions = self
            .store
            .list_own_live_sessions(&user_id)
            .await
            .map_err(internal)?
            .into_iter()
            .map(|s| OwnSession {
                handle: s.handle,
                workspace: s.workspace,
                git_branch: s.git_branch,
                current_activity: s.current_activity,
                last_event_at: s.last_event_at,
            })
            .collect();

        let services = ConfigLoader::load().map_err(internal)?;
        let grants = resolve_grants(self.store.db_pool(), &services, &identity)
            .await
            .map_err(internal)?;

        let report = WhoamiReport {
            user: WhoamiUser {
                id: identity.id,
                email: identity.email,
                display_name: identity.display_name,
                roles: identity.roles.clone(),
                department: identity.department,
            },
            odoo,
            grants,
            sessions,
            generated_at: chrono::Utc::now(),
        };
        let body = serde_json::to_string_pretty(&report).map_err(internal)?;
        let summary = format!(
            "{} · roles {} · odoo {} · {} plugin(s), {} server(s), {} skill(s)",
            report.user.email,
            identity.roles.join(","),
            if report.odoo.linked {
                "linked"
            } else {
                "not linked"
            },
            report.grants.plugins.len(),
            report.grants.mcp_servers.len(),
            report.grants.skills.len(),
        );
        Ok((text_artifact("Who Am I", &body), summary))
    }
}

async fn resolve_grants(
    db: &systemprompt::database::DbPool,
    services: &ServicesConfig,
    identity: &IdentityRow,
) -> Result<WhoamiGrants, systemprompt::security::authz::AuthzError> {
    let repo = AccessControlRepository::new(db)?;
    let sources = ChainSources::from_services(services);
    let index = ParentChainIndex::load(&repo, sources.clone()).await?;

    let marketplace_id = sources
        .marketplace
        .as_ref()
        .map(|m| m.id.as_str().to_owned());
    let plugin_ids: Vec<String> = sources.plugins.iter().map(|id| id.as_str().to_owned()).collect();
    let mcp_ids: Vec<String> = services
        .mcp_servers
        .iter()
        .filter(|(_, d)| d.enabled)
        .map(|(name, _)| name.clone())
        .collect();
    let skill_ids: Vec<String> = sources
        .skill_owners
        .keys()
        .map(|id| id.as_str().to_owned())
        .collect();

    let allowed = |kind: EntityKind, ids: &[String]| {
        let index = &index;
        let repo = &repo;
        let ids = ids.to_vec();
        async move {
            allowed_ids(
                repo,
                BulkKeepQuery {
                    user_id: &identity.id,
                    roles: &identity.roles,
                    kind,
                    ids: &ids,
                    chains: index,
                    attributes: &NO_SUBJECT_ATTRIBUTES,
                    dimensions: &[],
                },
            )
            .await
        }
    };

    let marketplace_ids: Vec<String> = marketplace_id.iter().cloned().collect();
    let marketplaces = allowed(EntityKind::Marketplace, &marketplace_ids).await?;
    let plugins = allowed(EntityKind::Plugin, &plugin_ids).await?;
    let mcp_servers = allowed(EntityKind::McpServer, &mcp_ids).await?;
    let skills = allowed(EntityKind::Skill, &skill_ids).await?;

    let own_plugins = repo
        .list_rules_bulk(EntityKind::Plugin, &plugin_ids)
        .await?;
    let own_mcp = repo
        .list_rules_bulk(EntityKind::McpServer, &mcp_ids)
        .await?;
    let own_skills = repo.list_rules_bulk(EntityKind::Skill, &skill_ids).await?;
    let has_own = |rules: &std::collections::HashMap<String, Vec<_>>, id: &str| {
        rules.get(id).is_some_and(|r| !r.is_empty())
    };

    let via_marketplace = || {
        marketplace_id
            .clone()
            .map_or(GrantSource::Own, GrantSource::Marketplace)
    };
    let granted = |ids: &[String], allowed: &HashSet<String>, via: &dyn Fn(&str) -> GrantSource| {
        ids.iter()
            .filter(|id| allowed.contains(*id))
            .map(|id| GrantedEntity {
                id: id.clone(),
                via: via(id),
            })
            .collect::<Vec<_>>()
    };

    Ok(WhoamiGrants {
        marketplaces: granted(&marketplace_ids, &marketplaces, &|_| GrantSource::Own),
        plugins: granted(&plugin_ids, &plugins, &|id| {
            if has_own(&own_plugins, id) {
                GrantSource::Own
            } else {
                via_marketplace()
            }
        }),
        mcp_servers: granted(&mcp_ids, &mcp_servers, &|id| {
            if has_own(&own_mcp, id) {
                GrantSource::Own
            } else {
                via_marketplace()
            }
        }),
        skills: granted(&skill_ids, &skills, &|id| {
            if has_own(&own_skills, id) {
                return GrantSource::Own;
            }
            // `SkillId` and `PluginId` impl `Borrow<str>`, so the map still
            // takes the plain `&str` that `granted` hands this closure — only
            // the value type changed when core made these ids typed.
            let owners: &BTreeSet<PluginId> = match sources.skill_owners.get(id) {
                Some(owners) => owners,
                None => return via_marketplace(),
            };
            owners
                .iter()
                .find(|owner| plugins.contains(owner.as_str()))
                .map(|owner| owner.as_str().to_owned())
                .map_or_else(via_marketplace, GrantSource::Plugin)
        }),
    })
}
