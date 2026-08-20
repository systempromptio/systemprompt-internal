//! Pure MIME-to-document shaping: RFC 822 bytes in, knowledge-bank row
//! fields out. No IMAP, no database — everything here is testable offline.

use mail_parser::{MessageParser, MimeHeaders};

// Why: the knowledge bank caps content at 2 MiB; the headroom keeps the
// header block this module prepends from pushing a maximal body over it.
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024 - 4096;

/// One parsed inbox message, reduced to the fields the knowledge bank keeps.
#[derive(Debug, Clone)]
pub struct CapturedEmail {
    pub mime_message_id: String,
    pub subject: String,
    pub from: String,
    pub to: String,
    pub date: String,
    pub body: String,
    pub attachment_names: Vec<String>,
}

#[must_use]
pub fn captured_from_rfc822(raw: &[u8], fallback_id: &str) -> Option<CapturedEmail> {
    let message = MessageParser::default().parse(raw)?;

    let mime_message_id = message
        .message_id()
        .map_or_else(|| fallback_id.to_owned(), str::to_owned);

    let subject = message
        .subject()
        .map_or_else(|| "(no subject)".to_owned(), str::to_owned);

    let from = address_line(message.from());
    let to = address_line(message.to());

    let date = message
        .date()
        .map_or_else(String::new, mail_parser::DateTime::to_rfc3339);

    let mut body = message
        .body_text(0)
        .map_or_else(String::new, std::borrow::Cow::into_owned);
    truncate_utf8(&mut body, MAX_BODY_BYTES);

    let attachment_names = message
        .attachments()
        .map(|a| a.attachment_name().unwrap_or("(unnamed)").to_owned())
        .collect();

    Some(CapturedEmail {
        mime_message_id,
        subject,
        from,
        to,
        date,
        body,
        attachment_names,
    })
}

#[must_use]
pub fn render_document(email: &CapturedEmail) -> String {
    let mut content = String::new();
    content.push_str(&format!("From: {}\n", email.from));
    content.push_str(&format!("To: {}\n", email.to));
    if !email.date.is_empty() {
        content.push_str(&format!("Date: {}\n", email.date));
    }
    content.push_str(&format!("Message-ID: {}\n", email.mime_message_id));
    if !email.attachment_names.is_empty() {
        content.push_str(&format!(
            "Attachments ({}): {}\n",
            email.attachment_names.len(),
            email.attachment_names.join(", ")
        ));
    }
    content.push('\n');
    content.push_str(&email.body);
    content
}

#[must_use]
pub fn metadata_json(email: &CapturedEmail) -> serde_json::Value {
    serde_json::json!({
        "message_id": email.mime_message_id,
        "from": email.from,
        "to": email.to,
        "date": email.date,
        "attachments": email.attachment_names,
    })
}

fn address_line(addr: Option<&mail_parser::Address<'_>>) -> String {
    let Some(addr) = addr else {
        return String::new();
    };
    let parts: Vec<String> = addr
        .iter()
        .map(|a| match (a.name(), a.address()) {
            (Some(name), Some(email)) => format!("{name} <{email}>"),
            (None, Some(email)) => email.to_owned(),
            (Some(name), None) => name.to_owned(),
            (None, None) => String::new(),
        })
        .filter(|s| !s.is_empty())
        .collect();
    parts.join(", ")
}

// Why: cutting mid-code-point would panic in String::truncate; back up to a
// char boundary before cutting.
fn truncate_utf8(s: &mut String, max: usize) {
    if s.len() <= max {
        return;
    }
    let mut cut = max;
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    s.truncate(cut);
    s.push_str("\n[truncated]");
}
