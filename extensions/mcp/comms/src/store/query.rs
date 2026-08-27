//! Address parsing and the delivery-class decision.
//!
//! Both are pure so the rule that matters — a message reaches a running
//! conversation only when it names that session — is decided in one place and
//! testable without a database.

use crate::error::CommsError;

pub const MAX_BODY_BYTES: usize = 8_000;
pub const DEFAULT_INBOX_LIMIT: i64 = 50;
pub const MAX_INBOX_LIMIT: i64 = 200;
pub const INBOX_SCOPE: &str = "inbox";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Address {
    User(String),
    Session { user: String, handle: String },
    Channel(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryClass {
    Inbox,
    Session,
    Urgent,
}

impl DeliveryClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inbox => "inbox",
            Self::Session => "session",
            Self::Urgent => "urgent",
        }
    }
}

// Why: a bare word is rejected rather than guessed at — silently reading `ed`
// as `@ed` would make `crm` mean a person when the sender meant the channel.
pub fn parse_address(raw: &str) -> Result<Address, CommsError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(CommsError::Invalid("`to` is required".to_owned()));
    }

    if let Some(channel) = trimmed.strip_prefix('#') {
        let slug = normalize_segment(channel);
        return if slug.is_empty() {
            Err(CommsError::Invalid(format!("invalid channel: `{raw}`")))
        } else {
            Ok(Address::Channel(slug))
        };
    }

    let Some(rest) = trimmed.strip_prefix('@') else {
        return Err(CommsError::Invalid(format!(
            "`{raw}` is not an address; use @user, @user/session-handle, or #channel"
        )));
    };

    if let Some((user, handle)) = rest.split_once('/') {
        let user = normalize_segment(user);
        let handle = normalize_handle(handle);
        if user.is_empty() || handle.is_empty() {
            return Err(CommsError::Invalid(format!("invalid address: `{raw}`")));
        }
        Ok(Address::Session { user, handle })
    } else {
        let user = normalize_segment(rest);
        if user.is_empty() {
            return Err(CommsError::Invalid(format!("invalid address: `{raw}`")));
        }
        Ok(Address::User(user))
    }
}

fn normalize_segment(segment: &str) -> String {
    segment
        .trim()
        .chars()
        .filter_map(|c| match c {
            'a'..='z' | '0'..='9' | '-' | '_' | '.' => Some(c),
            'A'..='Z' => Some(c.to_ascii_lowercase()),
            _ => None,
        })
        .collect()
}

// Why: handles keep `#` and `:`, which disambiguate concurrent sessions and
// branches (`odoo-crm#2`); the plain segment rules would strip both.
fn normalize_handle(segment: &str) -> String {
    segment
        .trim()
        .chars()
        .filter_map(|c| match c {
            'a'..='z' | '0'..='9' | '-' | '_' | '.' | '#' | ':' => Some(c),
            'A'..='Z' => Some(c.to_ascii_lowercase()),
            _ => None,
        })
        .collect()
}

// Why: a session-addressed message whose target is not live degrades to
// `inbox` rather than failing, so a sender never has to check whether a peer is
// online before writing to them.
#[must_use]
pub const fn classify(
    address_is_session: bool,
    target_is_live: bool,
    urgent: bool,
) -> DeliveryClass {
    if urgent {
        return DeliveryClass::Urgent;
    }
    if address_is_session && target_is_live {
        return DeliveryClass::Session;
    }
    DeliveryClass::Inbox
}

pub fn check_body(body: &str) -> Result<(), CommsError> {
    if body.trim().is_empty() {
        return Err(CommsError::Invalid("message body is empty".to_owned()));
    }
    if body.len() > MAX_BODY_BYTES {
        return Err(CommsError::TooLarge(format!(
            "message body is {} bytes; the limit is {MAX_BODY_BYTES}",
            body.len()
        )));
    }
    Ok(())
}

#[must_use]
pub fn clamp_limit(requested: Option<u32>) -> i64 {
    requested.map_or(DEFAULT_INBOX_LIMIT, |v| {
        i64::from(v).clamp(1, MAX_INBOX_LIMIT)
    })
}

#[must_use]
pub fn channel_scope(slug: &str) -> String {
    format!("channel:{slug}")
}
