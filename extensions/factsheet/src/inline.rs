//! Inline rich text.
//!
//! The sheets need a handful of inline forms — an accent span, bold, code, and
//! plain text. They are modelled as data rather than accepted as raw HTML
//! fragments because lead-derived copy flows into these fields: a company name
//! carrying `<script>` must land on the page as text, not as markup. Nothing
//! here can emit a tag the author did not name.
//!
//! # Why a span is a struct with a tone, not an enum of variants
//!
//! A span used to be an externally tagged enum (`- accent: "..."`). That reads
//! well in hand-written YAML and cannot survive a round trip: `serde_yaml`
//! writes such a variant back as a YAML tag, and a tagged value cannot be
//! deserialised inside an untagged enum. Since the whole editing model is
//! serialise the document, change it, deserialise it again, a shape that only
//! parses in one direction is not usable. Tone-as-a-field round-trips, and it
//! matches how the ledger and invoice blocks already model emphasis.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Span {
    pub text: String,
    #[serde(default)]
    pub tone: SpanTone,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SpanTone {
    #[default]
    Plain,
    /// Brand-orange emphasis — the house device for the second half of a
    /// headline or the payoff clause of a claim.
    Accent,
    Bold,
    Italic,
    Code,
    /// De-emphasised detail sitting inside a heavier line — a qualifier after a
    /// product name, for instance.
    Muted,
    /// Ink-coloured bold inside a prose row — the emphasis used in the provider
    /// strip and the off-agreement callout.
    Key,
}

/// Either a bare string (the common case) or a sequence of spans.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Inline {
    Plain(String),
    Spans(Vec<Span>),
}

impl Default for Inline {
    fn default() -> Self {
        Self::Plain(String::new())
    }
}

impl Inline {
    /// Render to HTML, escaping every author-supplied character.
    pub fn to_html(&self) -> String {
        match self {
            Self::Plain(text) => escape(text),
            Self::Spans(spans) => spans.iter().map(Span::to_html).collect(),
        }
    }

    /// Plain-text length, ignoring markup.
    pub fn text_len(&self) -> usize {
        match self {
            Self::Plain(text) => text.chars().count(),
            Self::Spans(spans) => spans.iter().map(|span| span.text.chars().count()).sum(),
        }
    }
}

impl Span {
    fn to_html(&self) -> String {
        let escaped = escape(&self.text);
        match self.tone {
            SpanTone::Plain => escaped,
            SpanTone::Accent => format!("<span class=\"accent\">{escaped}</span>"),
            SpanTone::Bold => format!("<b>{escaped}</b>"),
            SpanTone::Italic => format!("<em>{escaped}</em>"),
            SpanTone::Muted => format!("<span class=\"muted\">{escaped}</span>"),
            SpanTone::Code => format!("<code>{escaped}</code>"),
            SpanTone::Key => format!("<span class=\"g\">{escaped}</span>"),
        }
    }
}

/// Escape the five characters that can break out of text or an attribute.
pub fn escape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}
