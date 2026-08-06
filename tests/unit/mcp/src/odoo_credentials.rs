//! The sealed-credential format, asserted across both sides of it.
//!
//! The admin plane seals a user's Odoo API key into `odoo_identity`; the odoo
//! MCP server, a different binary, opens it. Nothing but this test holds the
//! two implementations to the same framing — a mismatch would surface only at
//! runtime, as every tool call failing for every linked user.

use systemprompt_mcp_odoo::error::OdooError;
use systemprompt_mcp_odoo::identity::{NOT_LINKED_MESSAGE, open_api_key};
use systemprompt_web_admin::repositories::users::odoo_identity::{open_with, seal_with};

const KEY: [u8; 32] = [7u8; 32];

#[test]
fn the_mcp_server_opens_what_the_admin_plane_sealed() {
    let sealed = seal_with(&KEY, "odoo-api-key").expect("sealing succeeds");

    let opened = open_api_key(&KEY, &sealed).expect("the MCP side opens it");

    assert_eq!(opened, "odoo-api-key");
}

#[test]
fn the_admin_plane_opens_its_own_sealed_value() {
    let sealed = seal_with(&KEY, "odoo-api-key").expect("sealing succeeds");

    assert_eq!(
        open_with(&KEY, &sealed).expect("round trip succeeds"),
        "odoo-api-key"
    );
}

#[test]
fn sealing_the_same_key_twice_produces_different_bytes() {
    let first = seal_with(&KEY, "same").expect("sealing succeeds");
    let second = seal_with(&KEY, "same").expect("sealing succeeds");

    assert_ne!(
        first, second,
        "a per-row nonce is what stops two users' identical API keys looking identical at rest"
    );
}

#[test]
fn a_wrong_key_does_not_open_the_credential() {
    let sealed = seal_with(&KEY, "odoo-api-key").expect("sealing succeeds");

    let err = open_api_key(&[9u8; 32], &sealed).expect_err("a wrong key must fail");

    assert!(matches!(err, OdooError::Internal(_)));
}

#[test]
fn a_truncated_blob_is_rejected_before_decryption() {
    let err = open_api_key(&KEY, "00112233").expect_err("too short to hold a nonce");

    assert!(matches!(err, OdooError::Internal(_)));
}

#[test]
fn non_hex_storage_is_rejected() {
    let err = open_api_key(&KEY, "not hex at all").expect_err("not hex");

    assert!(matches!(err, OdooError::Internal(_)));
}

#[test]
fn the_not_linked_message_names_the_page_that_fixes_it() {
    assert!(
        NOT_LINKED_MESSAGE.contains("/admin/profile"),
        "an agent told only \"not linked\" will look for a permissions problem"
    );
}
