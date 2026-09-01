//! The `From:` display line the ingestion job stored, reduced to a name and an
//! address.
//!
//! Ingestion flattened the parsed address to `Name <a@b>` text, so the split
//! is undone here rather than by re-parsing the MIME.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Sender {
    pub name: Option<String>,
    pub email: String,
}

impl Sender {
    #[must_use]
    pub fn display(&self) -> String {
        self.name.as_ref().map_or_else(
            || self.email.clone(),
            |name| format!("{name} <{}>", self.email),
        )
    }
}

#[must_use]
pub fn parse_mailbox(line: &str) -> Option<Sender> {
    // Why: a multi-recipient line keeps only the first mailbox — the sender
    // of an inbound email is one person even when the header lists more.
    let first = line.split(',').next()?.trim();
    if first.is_empty() {
        return None;
    }
    if let Some(open) = first.rfind('<') {
        let close = first[open..].find('>').map_or(first.len(), |c| open + c);
        let email = first[open + 1..close].trim().to_ascii_lowercase();
        if email.is_empty() {
            return None;
        }
        let name = first[..open].trim().trim_matches('"').trim();
        return Some(Sender {
            name: (!name.is_empty()).then(|| name.to_owned()),
            email,
        });
    }
    if first.contains('@') {
        return Some(Sender {
            name: None,
            email: first.to_ascii_lowercase(),
        });
    }
    None
}
