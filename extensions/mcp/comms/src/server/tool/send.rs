//! The `comms_send` handler: the only write, and where the delivery class is
//! decided.

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext as SysRequestContext;

use crate::store::{Address, CommsStore, NewMessage, check_body, classify, parse_address};
use crate::tools::{SendInput, TOOL_SEND};
use systemprompt::identifiers::{SessionId, UserId};

use super::common::{internal, invalid, text_artifact};

pub(super) struct SendHandler {
    pub(super) store: CommsStore,
}

impl McpToolHandler for SendHandler {
    type Input = SendInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_SEND
    }

    fn description(&self) -> &'static str {
        "Send a message to a person, a session, or a channel."
    }

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &SysRequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(Self::Output, String), McpError> {
        check_body(&input.body).map_err(invalid)?;
        let address = parse_address(&input.to).map_err(invalid)?;
        let urgent = input.urgent.unwrap_or(false);

        // Why: attribution is taken from the authenticated caller, never the
        // payload — a client that could name its own sender could forge any
        // message in the system.
        let sender_user_id = ctx.user_id().clone();
        let sender_session_id = ctx.session_id().clone();
        let sender_handle = self
            .store
            .find_session_handle(&sender_session_id)
            .await
            .map_err(internal)?;

        let resolved = resolve_destination(&self.store, address).await?;

        let class = classify(resolved.is_session, resolved.is_live, urgent);
        let sent = self
            .store
            .insert_message(&NewMessage {
                sender_user_id: &sender_user_id,
                sender_session_id: Some(&sender_session_id),
                sender_handle: sender_handle.as_deref(),
                channel_id: resolved.channel_id.as_deref(),
                recipient_user_id: resolved.user_id.as_ref(),
                recipient_session_id: resolved.session_id.as_ref(),
                delivery_class: class.as_str(),
                body: &input.body,
                thread_id: input.thread_id.as_deref(),
            })
            .await
            .map_err(internal)?;

        if let Some(recipient) = resolved.user_id.as_ref() {
            super::fanout::announce(&super::fanout::Announcement {
                message_id: &sent.id,
                recipient,
                session_id: resolved.session_id.as_ref(),
                sender: &sender_user_id,
                class,
                body: &input.body,
            })
            .await;
        }

        let note = if resolved.is_session && !resolved.is_live {
            " (that session is idle, so it went to their inbox instead)"
        } else {
            ""
        };
        let summary = format!("Sent to {} as {}{note}", resolved.label, class.as_str());
        let body = format!(
            "{summary}\n\nid: {}\nsent: {}\n\n{}",
            sent.id,
            sent.created_at.to_rfc3339(),
            input.body
        );
        Ok((text_artifact("Message Sent", &body), summary))
    }
}

struct Destination {
    channel_id: Option<String>,
    user_id: Option<UserId>,
    session_id: Option<SessionId>,
    is_session: bool,
    is_live: bool,
    label: String,
}

async fn resolve_destination(
    store: &CommsStore,
    address: Address,
) -> Result<Destination, McpError> {
    let mut d = Destination {
        channel_id: None,
        user_id: None,
        session_id: None,
        is_session: false,
        is_live: false,
        label: String::new(),
    };

    match address {
        Address::Channel(slug) => {
            let id = store
                .find_channel_id(&slug)
                .await
                .map_err(internal)?
                .ok_or_else(|| invalid(format!("no channel #{slug}")))?;
            d.label = format!("#{slug}");
            d.channel_id = Some(id);
        },
        Address::User(name) => {
            d.user_id = Some(lookup_user(store, &name).await?);
            d.label = format!("@{name}");
        },
        Address::Session { user, handle } => {
            let user_id = lookup_user(store, &user).await?;
            d.is_session = true;
            d.label = format!("@{user}/{handle}");
            if let Some(target) = store
                .find_live_session(&user_id, &handle)
                .await
                .map_err(internal)?
            {
                d.is_live = true;
                d.session_id = Some(target.session_id);
            }
            d.user_id = Some(user_id);
        },
    }

    Ok(d)
}

async fn lookup_user(store: &CommsStore, name: &str) -> Result<UserId, McpError> {
    store
        .find_user_by_name(name)
        .await
        .map_err(internal)?
        .ok_or_else(|| invalid(format!("no user @{name}")))
}
