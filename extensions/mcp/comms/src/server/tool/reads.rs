//! The four read handlers: inbox, history, channels, and the session
//! directory.

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext as SysRequestContext;

use crate::store::{Address, CommsStore, clamp_limit, parse_address};
use crate::tools::{
    ChannelsInput, HistoryInput, InboxInput, SessionsInput, TOOL_CHANNELS, TOOL_HISTORY,
    TOOL_INBOX, TOOL_SESSIONS,
};

use super::common::{internal, invalid, text_artifact};
use super::render::{channel_list, message_list, session_list};

pub(super) struct InboxHandler {
    pub(super) store: CommsStore,
}

impl McpToolHandler for InboxHandler {
    type Input = InboxInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_INBOX
    }

    fn description(&self) -> &'static str {
        "Read unread messages for this session."
    }

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &SysRequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(Self::Output, String), McpError> {
        let user_id = ctx.user_id().clone();
        let session_id = ctx.session_id().clone();
        let limit = clamp_limit(input.limit);

        let messages = self
            .store
            .list_inbox(&user_id, &session_id, limit)
            .await
            .map_err(internal)?;

        if !input.peek.unwrap_or(false) && !messages.is_empty() {
            self.store
                .mark_inbox_read(&user_id, &session_id)
                .await
                .map_err(internal)?;
        }

        let summary = format!("{} unread message(s)", messages.len());
        let body = message_list(&messages);
        Ok((text_artifact("Inbox", &body), summary))
    }
}

pub(super) struct HistoryHandler {
    pub(super) store: CommsStore,
}

impl McpToolHandler for HistoryHandler {
    type Input = HistoryInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_HISTORY
    }

    fn description(&self) -> &'static str {
        "Read conversation history with a person or channel."
    }

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &SysRequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(Self::Output, String), McpError> {
        let limit = clamp_limit(input.limit);
        let address = parse_address(&input.with).map_err(invalid)?;
        let user_id = ctx.user_id().clone();

        let (messages, label) = match address {
            Address::Channel(slug) => (
                self.store
                    .list_channel_history(&slug, limit)
                    .await
                    .map_err(internal)?,
                format!("#{slug}"),
            ),
            Address::User(name) | Address::Session { user: name, .. } => {
                let peer = self
                    .store
                    .find_user_by_name(&name)
                    .await
                    .map_err(internal)?
                    .ok_or_else(|| invalid(format!("no user @{name}")))?;
                (
                    self.store
                        .list_direct_history(&user_id, &peer, limit)
                        .await
                        .map_err(internal)?,
                    format!("@{name}"),
                )
            },
        };

        let summary = format!("{} message(s) with {label}", messages.len());
        let body = message_list(&messages);
        Ok((text_artifact("Conversation History", &body), summary))
    }
}

pub(super) struct ChannelsHandler {
    pub(super) store: CommsStore,
}

impl McpToolHandler for ChannelsHandler {
    type Input = ChannelsInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_CHANNELS
    }

    fn description(&self) -> &'static str {
        "List channels visible to the caller."
    }

    async fn handle(
        &self,
        _input: Self::Input,
        ctx: &SysRequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(Self::Output, String), McpError> {
        let channels = self
            .store
            .list_channels(ctx.user_id())
            .await
            .map_err(internal)?;
        let summary = format!("{} channel(s) visible", channels.len());
        let body = channel_list(&channels);
        Ok((text_artifact("Channels", &body), summary))
    }
}

pub(super) struct SessionsHandler {
    pub(super) store: CommsStore,
}

impl McpToolHandler for SessionsHandler {
    type Input = SessionsInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_SESSIONS
    }

    fn description(&self) -> &'static str {
        "List live agent sessions and their handles."
    }

    async fn handle(
        &self,
        _input: Self::Input,
        _ctx: &SysRequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(Self::Output, String), McpError> {
        let sessions = self.store.list_live_sessions().await.map_err(internal)?;
        let summary = format!("{} live session(s)", sessions.len());
        let body = session_list(&sessions);
        Ok((text_artifact("Live Sessions", &body), summary))
    }
}
