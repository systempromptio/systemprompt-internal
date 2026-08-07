//! Turning Odoo chatter into text a model can read.
//!
//! `mail.message.body` is HTML written by Odoo's web editor: paragraph tags,
//! inline styling, and occasionally a whole quoted email thread. Handing that
//! to a model verbatim spends most of the context window on markup and invites
//! it to quote tags back at the user. Every chatter body therefore passes
//! through [`html_to_text`] before it is rendered or excerpted.
//!
//! This is deliberately not a parser. It is the smallest transformation that
//! makes Odoo's own output legible, and it assumes the input is Odoo's editor
//! rather than arbitrary hostile HTML — the values here are read back from a
//! record the acting user can already see.

/// Longest snippet [`snippet_around`] will return, in characters.
pub const SNIPPET_CHARS: usize = 200;

// Why: the entities Odoo's editor actually emits. A full entity table would be
// dead weight — anything unlisted survives as literal text, which reads worse
// than the character but never wrong.
const ENTITIES: [(&str, &str); 6] = [
    ("&nbsp;", " "),
    ("&amp;", "&"),
    ("&lt;", "<"),
    ("&gt;", ">"),
    ("&quot;", "\""),
    ("&#39;", "'"),
];

// Why: block-level tags become a space rather than vanishing, so
// "<p>a</p><p>b</p>" reads as "a b" and not "ab". Odoo separates chatter
// paragraphs this way.
fn is_block_break(tag: &str) -> bool {
    let name = tag
        .trim_start_matches('/')
        .split(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        name.as_str(),
        "p" | "br" | "div" | "li" | "tr" | "td" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
    )
}

/// Strip HTML markup from a chatter body and collapse its whitespace.
///
/// Tags are removed, block-level tags become spaces, and the handful of
/// entities Odoo emits are decoded. Runs of whitespace collapse to one space,
/// because Odoo's editor indents its markup and the indentation is not content.
#[must_use]
pub fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut tag = String::new();
    let mut in_tag = false;

    // Why: an unterminated `<` means the body was truncated mid-tag. Leaving
    // `in_tag` set discards the partial tag and keeps the text before it,
    // which is the half worth returning.
    for ch in html.chars() {
        match ch {
            '<' => {
                in_tag = true;
                tag.clear();
            },
            '>' if in_tag => {
                in_tag = false;
                if is_block_break(&tag) {
                    out.push(' ');
                }
            },
            _ if in_tag => tag.push(ch),
            _ => out.push(ch),
        }
    }

    for (entity, replacement) in ENTITIES {
        if out.contains(entity) {
            out = out.replace(entity, replacement);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// An excerpt of `text` centred on the first case-insensitive hit for `query`.
///
/// Capped at [`SNIPPET_CHARS`]; an ellipsis marks each end where text was
/// dropped. A query that does not appear falls back to the head of the text
/// rather than to nothing — Odoo matches on subject as well as body, so a hit
/// with no body match is normal, and an empty snippet would read as an empty
/// record.
#[must_use]
pub fn snippet_around(text: &str, query: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= SNIPPET_CHARS {
        return text.to_owned();
    }

    let start = match_start(text, query).map_or(0, |hit| {
        // Why: centre the hit, then clamp so a match near either end still
        // yields a full-width snippet instead of a short one.
        let half = SNIPPET_CHARS / 2;
        hit.saturating_sub(half)
            .min(chars.len().saturating_sub(SNIPPET_CHARS))
    });
    let end = (start + SNIPPET_CHARS).min(chars.len());

    let mut snippet = String::new();
    if start > 0 {
        snippet.push('…');
    }
    snippet.extend(&chars[start..end]);
    if end < chars.len() {
        snippet.push('…');
    }
    snippet
}

// Why: the character offset of the first case-insensitive hit. Lowercasing can
// change byte length, so the search runs over char vectors and returns a char
// index — a byte offset from `str::find` on the lowercased copy would not line
// up with the original.
fn match_start(text: &str, query: &str) -> Option<usize> {
    let needle: Vec<char> = query.trim().to_lowercase().chars().collect();
    if needle.is_empty() {
        return None;
    }
    let haystack: Vec<char> = text.to_lowercase().chars().collect();
    haystack
        .windows(needle.len())
        .position(|window| window == needle.as_slice())
}
