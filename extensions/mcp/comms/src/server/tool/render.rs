//! Rendering for the comms tools.
//!
//! Every tool returns a text body alongside its artifact because hosts that
//! did not negotiate the UI extension — Codex among them — show the summary
//! and nothing else. The body has to stand on its own.

use crate::store::{ChannelRow, MessageRow, SessionRow};

pub const NO_MESSAGES: &str = "No new messages.";
pub const NO_CHANNELS: &str = "No channels are visible to you yet.";
pub const NO_SESSIONS: &str = "No agent sessions are live right now.";

fn origin(message: &MessageRow) -> String {
    match (
        message.channel_slug.as_deref(),
        message.sender_handle.as_deref(),
    ) {
        (Some(slug), Some(handle)) => format!("#{slug} · @{}/{handle}", message.sender_user_id),
        (Some(slug), None) => format!("#{slug} · @{}", message.sender_user_id),
        (None, Some(handle)) => format!("@{}/{handle}", message.sender_user_id),
        (None, None) => format!("@{}", message.sender_user_id),
    }
}

#[must_use]
pub fn message_list(messages: &[MessageRow]) -> String {
    if messages.is_empty() {
        return NO_MESSAGES.to_owned();
    }
    messages
        .iter()
        .map(|m| {
            format!(
                "- **{}** — {} · {}\n  {}",
                origin(m),
                m.created_at.to_rfc3339(),
                m.delivery_class,
                m.body
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[must_use]
pub fn channel_list(channels: &[ChannelRow]) -> String {
    if channels.is_empty() {
        return NO_CHANNELS.to_owned();
    }
    channels
        .iter()
        .map(|c| {
            format!(
                "- **#{}** — {}{} · {} member(s){}",
                c.slug,
                c.name,
                c.description
                    .as_deref()
                    .map_or_else(String::new, |d| format!(" — {d}")),
                c.member_count.unwrap_or(0),
                if c.urgent { " · urgent" } else { "" }
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[must_use]
pub fn session_list(sessions: &[SessionRow]) -> String {
    if sessions.is_empty() {
        return NO_SESSIONS.to_owned();
    }
    sessions
        .iter()
        .map(|s| {
            format!(
                "- **@{}/{}** — {}{}{} · last seen {}",
                s.user_id,
                s.handle,
                s.display_name.as_deref().unwrap_or(s.user_id.as_str()),
                s.workspace
                    .as_deref()
                    .map_or_else(String::new, |w| format!(" · {w}")),
                s.git_branch
                    .as_deref()
                    .map_or_else(String::new, |b| format!(":{b}")),
                s.last_event_at
                    .map_or_else(|| "—".to_owned(), |t| t.to_rfc3339()),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
