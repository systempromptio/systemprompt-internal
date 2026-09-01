//! Rendering a captured email as the HTML Odoo's chatter expects.
//!
//! `mail.message.body` is HTML, so the plain text is escaped and line-broken
//! rather than pasted; the ingestion job's own header block is dropped and
//! re-rendered from the typed metadata, and the whole thing is bounded so a
//! 2 MiB newsletter cannot become a 2 MiB chatter row.

use super::sender::Sender;

pub const MAX_BODY_CHARS: usize = 20_000;

/// What the renderer needs from a document.
#[derive(Debug, Clone, Copy)]
pub struct BodySource<'a> {
    pub sender: &'a Sender,
    pub subject: &'a str,
    pub received: &'a str,
    pub rfc5322_id: &'a str,
    pub content: &'a str,
    pub document_id: &'a str,
}

#[must_use]
pub fn chatter_body(source: &BodySource<'_>) -> String {
    let mut text = strip_ingest_headers(source.content);
    if let Some((idx, _)) = text.char_indices().nth(MAX_BODY_CHARS) {
        text = &text[..idx];
    }
    let mut html = String::with_capacity(text.len() + 512);
    html.push_str("<p><strong>From:</strong> ");
    html.push_str(&escape(&source.sender.display()));
    if !source.received.is_empty() {
        html.push_str("<br><strong>Received:</strong> ");
        html.push_str(&escape(source.received));
    }
    html.push_str("<br><strong>Subject:</strong> ");
    html.push_str(&escape(source.subject));
    html.push_str("</p><p>");
    html.push_str(&escape(text).replace('\n', "<br>"));
    if text.len() < strip_ingest_headers(source.content).len() {
        html.push_str("<br>[truncated]");
    }
    html.push_str("</p><p><em>Captured by brain@ · knowledge document ");
    html.push_str(&escape(source.document_id));
    html.push_str(" · Message-ID ");
    html.push_str(&escape(source.rfc5322_id));
    html.push_str("</em></p>");
    html
}

// Why: ingestion prepends `From:/To:/Date:/Message-ID:` lines and a blank
// line; the metadata column carries the same values typed, so the block is
// cut rather than shown twice.
fn strip_ingest_headers(content: &str) -> &str {
    content
        .split_once("\n\n")
        .filter(|(head, _)| head.starts_with("From:"))
        .map_or(content, |(_, body)| body)
        .trim()
}

#[must_use]
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}
