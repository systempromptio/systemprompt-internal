//! Preserve internal session addresses and live activity alongside usage
//! metrics.
use super::helpers;
use super::processing::ProcessInsertedEventParams;
use crate::repositories::dashboard::session_registry;
use crate::types::webhook::HookEvent;

pub(super) async fn record(params: &ProcessInsertedEventParams<'_>) {
    let cwd = params.payload.common.cwd.trim();
    let workspace = session_registry::derive_workspace(cwd);
    let result = sqlx::query!("UPDATE plugin_session_summaries SET cwd = COALESCE(cwd, NULLIF($2, '')), workspace = COALESCE(workspace, $3), last_event_at = NOW() WHERE session_id = $1", params.session_id.as_str(), cwd, workspace.as_deref())
        .execute(params.pool).await;
    if let Err(error) = result {
        tracing::warn!(%error, "Failed to update internal session workspace");
    }
    if let Some(workspace) = workspace {
        session_registry::assign_session_handle(params.pool, params.session_id, &workspace).await;
    }
    let activity = match &params.payload.event {
        HookEvent::UserPromptSubmit(data) if !data.prompt.is_empty() => {
            helpers::derive_title(&data.prompt)
        },
        _ => match params.tool_name {
            Some(tool) if !tool.is_empty() => tool.to_owned(),
            _ => return,
        },
    };
    session_registry::update_session_activity(params.pool, params.session_id, &activity).await;
}
