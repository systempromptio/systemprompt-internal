//! The attachment size gates, search domain, and rendering.
//!
//! The two limits are the interesting part. A size gate that is off by a factor
//! of 1024, or that measures the base64 string rather than the bytes it
//! decodes to, fails in a way nothing downstream reports: the upload just
//! succeeds or refuses at the wrong threshold.

use systemprompt_mcp_odoo::attachment::{
    MAX_INLINE_BYTES, MAX_UPLOAD_BYTES, attachment_domain, attachment_row, check_upload,
    too_large_notice,
};
use systemprompt_mcp_odoo::tools::inputs::AttachmentListInput;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

fn listing(model: Option<&str>, res_id: Option<i64>, query: Option<&str>) -> AttachmentListInput {
    AttachmentListInput {
        model: model.map(str::to_owned),
        res_id,
        query: query.map(str::to_owned),
        limit: None,
    }
}

#[test]
fn a_valid_payload_reports_its_decoded_length() {
    let encoded = STANDARD.encode(b"hello world");

    let size = check_upload(&encoded).expect("valid base64 is accepted");

    assert_eq!(
        size,
        11,
        "the gate measures decoded bytes, not the {} characters of base64",
        encoded.len()
    );
}

#[test]
fn a_payload_at_the_limit_is_accepted() {
    let encoded = STANDARD.encode(vec![0u8; MAX_UPLOAD_BYTES]);

    let size = check_upload(&encoded).expect("exactly at the limit is within it");

    assert_eq!(size, MAX_UPLOAD_BYTES);
}

#[test]
fn a_payload_one_byte_over_the_limit_is_refused() {
    let encoded = STANDARD.encode(vec![0u8; MAX_UPLOAD_BYTES + 1]);

    let err = check_upload(&encoded).expect_err("over the limit is refused");

    assert!(
        err.message.contains("upload limit"),
        "the refusal must say why: {}",
        err.message
    );
    assert!(
        err.message.contains("web UI"),
        "and where to go instead: {}",
        err.message
    );
}

#[test]
fn the_upload_limit_is_five_megabytes_of_bytes_not_kilobytes() {
    assert_eq!(
        MAX_UPLOAD_BYTES,
        5 * 1024 * 1024,
        "a factor-of-1024 slip here would silently accept or refuse the wrong files"
    );
}

#[test]
fn the_inline_limit_is_one_megabyte_and_well_under_the_upload_limit() {
    assert_eq!(MAX_INLINE_BYTES, 1024 * 1024);
    assert!(
        i64::try_from(MAX_UPLOAD_BYTES).is_ok_and(|upload| MAX_INLINE_BYTES < upload),
        "a file can be uploadable yet too large to hand back through the context window"
    );
}

#[test]
fn a_payload_that_is_not_base64_is_refused_before_any_upload() {
    let err = check_upload("this is not base64!!!").expect_err("garbage is refused");

    assert!(
        err.message.contains("not valid base64"),
        "got: {}",
        err.message
    );
}

#[test]
fn an_empty_payload_is_refused() {
    let err = check_upload("").expect_err("an empty file is not an upload");

    assert!(err.message.contains("empty"), "got: {}", err.message);
}

#[test]
fn surrounding_whitespace_does_not_invalidate_a_payload() {
    let encoded = format!("  {}\n", STANDARD.encode(b"padded"));

    assert_eq!(
        check_upload(&encoded).expect("trimmed before decoding"),
        6,
        "a client that pretty-prints its JSON should not fail the gate"
    );
}

#[test]
fn an_unfiltered_listing_has_an_empty_domain() {
    assert_eq!(
        attachment_domain(&listing(None, None, None)),
        serde_json::json!([]),
        "an unscoped list must not smuggle in a condition"
    );
}

#[test]
fn each_filter_contributes_one_leaf() {
    assert_eq!(
        attachment_domain(&listing(Some("crm.lead"), Some(42), Some("proposal"))),
        serde_json::json!([
            ["res_model", "=", "crm.lead"],
            ["res_id", "=", 42],
            ["name", "ilike", "%proposal%"]
        ]),
        "bare leaves concatenated are an implicit AND"
    );
}

#[test]
fn a_filename_query_is_wildcarded() {
    let domain = attachment_domain(&listing(None, None, Some("spec")));

    assert_eq!(domain[0][2], "%spec%");
}

#[test]
fn a_blank_query_is_not_a_filter() {
    assert_eq!(
        attachment_domain(&listing(None, None, Some("   "))),
        serde_json::json!([])
    );
}

#[test]
fn a_record_id_alone_is_permitted() {
    assert_eq!(
        attachment_domain(&listing(None, Some(7), None)),
        serde_json::json!([["res_id", "=", 7]]),
        "ids repeat across models, but a caller may genuinely want every model's 7"
    );
}

#[test]
fn an_attachment_row_leads_with_the_id_and_names_its_record() {
    let record = serde_json::json!({
        "id": 3,
        "name": "proposal.pdf",
        "mimetype": "application/pdf",
        "file_size": 20_480,
        "res_model": "crm.lead",
        "res_id": 42,
    });

    let row = attachment_row(&record);

    assert!(row.starts_with("- **[3] proposal.pdf**"), "got: {row}");
    assert!(row.contains("application/pdf") && row.contains("20480"));
    assert!(row.contains("crm.lead") && row.contains("42"));
}

#[test]
fn the_withheld_notice_gives_the_size_and_the_alternative() {
    let notice = too_large_notice(9_000_000);

    assert!(
        notice.contains("9000000"),
        "state the actual size: {notice}"
    );
    assert!(
        notice.contains(&MAX_INLINE_BYTES.to_string()),
        "and the limit it exceeded: {notice}"
    );
    assert!(
        notice.contains("Odoo web UI"),
        "a refusal without a route forward strands the caller: {notice}"
    );
}
