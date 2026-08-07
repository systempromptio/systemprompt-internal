//! Email → knowledge-document shaping: parsing, fallbacks, rendering, and
//! the truncation guard, all without IMAP or a database.

use systemprompt_knowledge_jobs::internals::{
    captured_from_rfc822, metadata_json, render_document,
};

const SIMPLE: &[u8] = b"From: Victor <victor@systemprompt.io>\r\n\
To: Brain <brain@systemprompt.io>\r\n\
Subject: Q3 pricing notes\r\n\
Date: Mon, 3 Aug 2026 10:00:00 +0000\r\n\
Message-ID: <abc123@mail.example>\r\n\
Content-Type: text/plain\r\n\
\r\n\
We agreed to hold the enterprise tier at current pricing.\r\n";

#[test]
fn parses_headers_and_body() {
    let email = captured_from_rfc822(SIMPLE, "fallback").expect("parseable");
    assert_eq!(email.mime_message_id, "abc123@mail.example");
    assert_eq!(email.subject, "Q3 pricing notes");
    assert_eq!(email.from, "Victor <victor@systemprompt.io>");
    assert_eq!(email.to, "Brain <brain@systemprompt.io>");
    assert!(email.body.contains("enterprise tier"));
    assert!(email.attachment_names.is_empty());
}

#[test]
fn missing_message_id_uses_fallback() {
    let raw = b"From: a@b.c\r\nSubject: no id\r\n\r\nbody\r\n";
    let email =
        captured_from_rfc822(raw, "imap:brain@systemprompt.io:INBOX:42").expect("parseable");
    assert_eq!(email.mime_message_id, "imap:brain@systemprompt.io:INBOX:42");
}

#[test]
fn missing_subject_gets_placeholder() {
    let raw = b"From: a@b.c\r\nMessage-ID: <x@y>\r\n\r\nbody\r\n";
    let email = captured_from_rfc822(raw, "fb").expect("parseable");
    assert_eq!(email.subject, "(no subject)");
}

#[test]
fn rendered_document_carries_headers_for_search() {
    let email = captured_from_rfc822(SIMPLE, "fb").expect("parseable");
    let content = render_document(&email);
    assert!(content.starts_with("From: Victor <victor@systemprompt.io>\n"));
    assert!(content.contains("Message-ID: abc123@mail.example"));
    assert!(
        content
            .trim_end()
            .ends_with("We agreed to hold the enterprise tier at current pricing.")
    );
}

#[test]
fn metadata_json_round_trips_fields() {
    let email = captured_from_rfc822(SIMPLE, "fb").expect("parseable");
    let meta = metadata_json(&email);
    assert_eq!(meta["message_id"], "abc123@mail.example");
    assert_eq!(meta["from"], "Victor <victor@systemprompt.io>");
    assert_eq!(meta["attachments"].as_array().map(Vec::len), Some(0));
}

#[test]
fn oversized_body_is_truncated_not_refused() {
    let mut raw = Vec::new();
    raw.extend_from_slice(b"From: a@b.c\r\nMessage-ID: <big@y>\r\nSubject: big\r\n\r\n");
    raw.extend(std::iter::repeat_n(b'x', 3 * 1024 * 1024));
    let email = captured_from_rfc822(&raw, "fb").expect("parseable");
    assert!(email.body.len() < 2 * 1024 * 1024);
    assert!(email.body.ends_with("[truncated]"));
}
