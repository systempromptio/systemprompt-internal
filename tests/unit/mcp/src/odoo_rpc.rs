//! The JSON-RPC envelope: request shape in, result or fault out.
//!
//! Odoo signals an application-level refusal with HTTP 200 and an `error`
//! member, so envelope parsing is where "you may not do that" is distinguished
//! from "the call worked". Getting it wrong turns an access-rule refusal into a
//! silent empty result.

use systemprompt_mcp_odoo::client::rpc::{build_request, parse_response};
use systemprompt_mcp_odoo::error::OdooError;

#[test]
fn build_request_places_service_method_and_args_under_params() {
    let request = build_request(
        "object",
        "execute_kw",
        &[serde_json::json!("db"), serde_json::json!(7)],
    );

    assert_eq!(request["jsonrpc"], "2.0");
    assert_eq!(request["method"], "call");
    assert_eq!(request["params"]["service"], "object");
    assert_eq!(request["params"]["method"], "execute_kw");
    assert_eq!(
        request["params"]["args"],
        serde_json::json!(["db", 7]),
        "args are positional — Odoo reads them by index, not by name"
    );
}

#[test]
fn parse_response_returns_the_result_member() {
    let parsed = parse_response(r#"{"jsonrpc":"2.0","id":1,"result":[{"id":4}]}"#)
        .expect("a well-formed response parses");

    assert_eq!(parsed, serde_json::json!([{"id": 4}]));
}

#[test]
fn parse_response_treats_a_missing_result_as_null_not_an_error() {
    let parsed = parse_response(r#"{"jsonrpc":"2.0","id":1}"#).expect("an empty envelope parses");

    assert_eq!(parsed, serde_json::Value::Null);
}

#[test]
fn parse_response_prefers_the_inner_fault_message() {
    let body = r#"{"error":{"message":"Odoo Server Error","data":{"message":"You are not allowed to access 'Lead' records."}}}"#;

    let err = parse_response(body).expect_err("a fault is an error");

    assert!(
        matches!(&err, OdooError::Odoo(msg) if msg.contains("not allowed to access")),
        "the useful text lives in error.data.message; the outer message is a class name: {err}"
    );
}

#[test]
fn parse_response_falls_back_to_the_outer_message_when_there_is_no_inner_one() {
    let body = r#"{"error":{"message":"Access Denied"}}"#;

    let err = parse_response(body).expect_err("a fault is an error");

    assert!(matches!(&err, OdooError::Odoo(msg) if msg == "Access Denied"));
}

#[test]
fn parse_response_reports_unparseable_bodies_as_transport_failures() {
    let err = parse_response("<html>502 Bad Gateway</html>").expect_err("html is not an envelope");

    assert!(
        matches!(err, OdooError::Transport(_)),
        "a proxy error page is the transport failing, not Odoo refusing"
    );
}
