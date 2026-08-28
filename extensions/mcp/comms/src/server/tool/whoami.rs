//! The `comms_whoami` handler.
//!
//! Grants come from core's `ParentChainIndex` over the live services config
//! and `access_control_rules` — the resolver the bridge manifest is filtered
//! with — so what this reports and what the bridge mounted are one answer.
//! Subject attributes beyond `user` and `role` (a department dimension the
//! admin extension declares) are not evaluated here; a grant conferred only
//! by such a dimension shows on the admin access matrix, not in this panel.



use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::loader::ConfigLoader;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext as SysRequestContext;

use crate::store::CommsStore;
use crate::tools::{TOOL_WHOAMI, WhoamiInput};
use crate::whoami::{OdooLinkStatus, OwnSession, WhoamiReport, WhoamiUser};

use super::common::{internal, text_artifact};
use super::whoami_grants::resolve_grants;

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
