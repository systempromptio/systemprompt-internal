//! Notifying a recipient that a message is waiting.
//!
//! The event carries ids and a short preview, never the body. A client that
//! wants the message fetches it through `comms_inbox`, so the SSE leg never
//! becomes a second, unaudited path to message content — and a preview that
//! leaks into a log is a preview, not the message.
//!
//! Only `session` and `urgent` classes are announced. An `inbox`-class message
//! is deliberately silent: raising an unread count is the whole of its
//! contract, and announcing it would reintroduce the interruption the delivery
//! classes exist to prevent.

use systemprompt::events::EventRouter;
use systemprompt::identifiers::{SessionId, UserId};
use systemprompt::models::{AgUiEventBuilder, CustomPayload, GenericCustomPayload};

use crate::store::DeliveryClass;

const PREVIEW_CHARS: usize = 120;
pub const EVENT_NAME: &str = "comms.message";

fn preview(body: &str) -> String {
    let trimmed = body.trim();
    match trimmed.char_indices().nth(PREVIEW_CHARS) {
        Some((idx, _)) => format!("{}…", &trimmed[..idx]),
        None => trimmed.to_owned(),
    }
}

#[derive(Debug)]
pub struct Announcement<'a> {
    pub message_id: &'a str,
    pub recipient: &'a UserId,
    pub session_id: Option<&'a SessionId>,
    pub sender: &'a UserId,
    pub class: DeliveryClass,
    pub body: &'a str,
}

pub async fn announce(a: &Announcement<'_>) {
    if matches!(a.class, DeliveryClass::Inbox) {
        return;
    }

    let event = AgUiEventBuilder::custom(CustomPayload::Generic(GenericCustomPayload {
        name: EVENT_NAME.to_owned(),
        value: serde_json::json!({
            "messageId": a.message_id,
            "sessionId": a.session_id.map(SessionId::as_str),
            "from": a.sender.as_str(),
            "deliveryClass": a.class.as_str(),
            "preview": preview(a.body),
        }),
    }));

    let (agui, ctx) = EventRouter::route_agui(a.recipient, event).await;
    tracing::debug!(
        message_id = %a.message_id,
        recipient = %a.recipient,
        delivered = agui + ctx,
        "comms message announced"
    );
}
