//! `OdooError`'s code, status and retryability — and how it reaches a caller.
//!
//! The distinction that matters here is which side is at fault. "You have not
//! linked Odoo" is the caller's to fix and must reach them as an invalid
//! request; a fault from Odoo itself must not be dressed up as one.

use axum::http::StatusCode;
use systemprompt::traits::ExtensionError;
use systemprompt_mcp_odoo::error::OdooError;

#[test]
fn codes_and_statuses_separate_the_three_failing_parties() {
    let cases = [
        (
            OdooError::NotConfigured("no url".to_owned()),
            "NOT_CONFIGURED",
            StatusCode::SERVICE_UNAVAILABLE,
        ),
        (
            OdooError::NotLinked("link it".to_owned()),
            "NOT_LINKED",
            StatusCode::FORBIDDEN,
        ),
        (
            OdooError::Odoo("access denied".to_owned()),
            "ODOO_REJECTED",
            StatusCode::BAD_GATEWAY,
        ),
        (
            OdooError::Transport("connection refused".to_owned()),
            "UPSTREAM_UNAVAILABLE",
            StatusCode::SERVICE_UNAVAILABLE,
        ),
        (
            OdooError::Internal("bug".to_owned()),
            "INTERNAL_ERROR",
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
    ];

    for (error, code, status) in cases {
        assert_eq!(error.code(), code);
        assert_eq!(error.status(), status);
    }
}

#[test]
fn only_a_transport_failure_is_worth_retrying_unchanged() {
    assert!(OdooError::Transport("timeout".to_owned()).is_retryable());
    assert!(!OdooError::NotLinked("link it".to_owned()).is_retryable());
    assert!(
        !OdooError::Odoo("access denied".to_owned()).is_retryable(),
        "retrying a refused call just refuses again"
    );
}

#[test]
fn a_missing_link_reaches_the_caller_as_something_they_can_fix() {
    let mcp_error: rmcp::ErrorData = OdooError::NotLinked("link it on /admin/profile".to_owned()).into();

    assert!(
        mcp_error.message.contains("/admin/profile"),
        "the actionable text must survive the conversion: {}",
        mcp_error.message
    );
}

#[test]
fn an_odoo_fault_is_not_reported_as_the_callers_mistake() {
    let mcp_error: rmcp::ErrorData = OdooError::Odoo("Access Denied".to_owned()).into();

    assert!(
        mcp_error.message.contains("Access Denied"),
        "got: {}",
        mcp_error.message
    );
}
