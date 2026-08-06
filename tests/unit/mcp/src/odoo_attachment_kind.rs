//! `attachment_add`'s two shapes: stored bytes, or a pointer to bytes held
//! elsewhere.
//!
//! The exactly-one-of rule is the part worth pinning. Accepting both would
//! store one and silently drop the other; accepting neither would create an
//! empty attachment that looks like a failed upload. Neither failure announces
//! itself, so both are asserted here rather than left to a live Odoo.

use systemprompt_mcp_odoo::attachment::{
    Upload, classify_upload, create_values, is_url_attachment,
};
use systemprompt_mcp_odoo::tools::inputs::AttachmentAddInput;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

fn input(content: Option<&str>, url: Option<&str>) -> AttachmentAddInput {
    AttachmentAddInput {
        model: "crm.lead".to_owned(),
        res_id: 42,
        filename: "proposal.pdf".to_owned(),
        content_base64: content.map(str::to_owned),
        url: url.map(str::to_owned),
        mimetype: None,
    }
}

#[test]
fn base64_content_classifies_as_a_stored_file() {
    let encoded = STANDARD.encode(b"hello");

    let upload = classify_upload(&input(Some(&encoded), None)).expect("valid payload");

    assert_eq!(
        upload,
        Upload::Binary {
            content_base64: encoded,
            size: 5
        }
    );
}

#[test]
fn a_url_classifies_as_a_pointer() {
    let upload =
        classify_upload(&input(None, Some("https://store.example.com/rec.mp4"))).expect("valid");

    assert_eq!(
        upload,
        Upload::Url("https://store.example.com/rec.mp4".to_owned())
    );
}

#[test]
fn giving_both_is_refused_rather_than_resolved() {
    let encoded = STANDARD.encode(b"hello");

    let err = classify_upload(&input(Some(&encoded), Some("https://example.com/x")))
        .expect_err("ambiguous input is refused");

    assert!(
        err.message.contains("not both"),
        "the caller has not decided what they are creating: {}",
        err.message
    );
}

#[test]
fn giving_neither_is_refused() {
    let err = classify_upload(&input(None, None)).expect_err("nothing to attach");

    assert!(
        err.message.contains("content_base64") && err.message.contains("url"),
        "name both ways out: {}",
        err.message
    );
}

#[test]
fn blank_strings_count_as_absent() {
    let err = classify_upload(&input(Some("   "), Some(""))).expect_err("both blank is neither");

    assert!(err.message.contains("Provide content_base64"), "got: {}", err.message);
}

#[test]
fn a_non_http_url_is_refused_before_it_becomes_a_dead_link() {
    let err = classify_upload(&input(None, Some("mailto:someone@example.com")))
        .expect_err("not a fetchable address");

    assert!(err.message.contains("http"), "got: {}", err.message);
}

#[test]
fn an_oversized_base64_body_is_still_refused_through_classify() {
    let encoded = STANDARD.encode(vec![0u8; 6 * 1024 * 1024]);

    let err = classify_upload(&input(Some(&encoded), None)).expect_err("over the cap");

    assert!(err.message.contains("upload limit"), "got: {}", err.message);
}

#[test]
fn the_binary_payload_carries_datas_and_declares_its_type() {
    let encoded = STANDARD.encode(b"hello");
    let input = input(Some(&encoded), None);
    let upload = classify_upload(&input).expect("valid");

    let values = create_values(&input, "proposal.pdf", &upload);

    assert_eq!(values["type"], "binary");
    assert_eq!(values["datas"], encoded);
    assert_eq!(values["res_model"], "crm.lead");
    assert_eq!(values["res_id"], 42);
    assert!(
        values.get("url").is_none(),
        "a stored file has no url: {values}"
    );
}

#[test]
fn the_url_payload_carries_url_and_no_datas() {
    let input = input(None, Some("https://store.example.com/rec.mp4"));
    let upload = classify_upload(&input).expect("valid");

    let values = create_values(&input, "Kickoff recording", &upload);

    assert_eq!(values["type"], "url");
    assert_eq!(values["url"], "https://store.example.com/rec.mp4");
    assert_eq!(values["name"], "Kickoff recording");
    assert!(
        values.get("datas").is_none(),
        "a pointer stores no bytes: {values}"
    );
}

#[test]
fn a_declared_mimetype_rides_along_with_a_stored_file_only() {
    let encoded = STANDARD.encode(b"hello");
    let mut binary = input(Some(&encoded), None);
    binary.mimetype = Some("application/pdf".to_owned());
    let binary_values = create_values(&binary, "p.pdf", &classify_upload(&binary).expect("ok"));
    assert_eq!(binary_values["mimetype"], "application/pdf");

    let mut link = input(None, Some("https://example.com/x"));
    link.mimetype = Some("video/mp4".to_owned());
    let link_values = create_values(&link, "rec", &classify_upload(&link).expect("ok"));
    assert!(
        link_values.get("mimetype").is_none(),
        "Odoo does not serve a link's bytes, so it cannot promise their type: {link_values}"
    );
}

#[test]
fn url_rows_are_recognisable_after_the_fact() {
    assert!(is_url_attachment(&serde_json::json!({ "type": "url" })));
    assert!(!is_url_attachment(&serde_json::json!({ "type": "binary" })));
    assert!(
        !is_url_attachment(&serde_json::json!({})),
        "an unknown row is treated as stored, which only risks an extra read"
    );
}
