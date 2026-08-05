//! `KnowledgeBankError`'s `ExtensionError` impl is what the HTTP layer turns
//! into a status line: each variant maps to a stable machine code and status,
//! and nothing this server produces is worth a client retry.

use axum::http::StatusCode;
use systemprompt::traits::ExtensionError;
use systemprompt_mcp_knowledge_bank::error::KnowledgeBankError;

#[test]
fn each_variant_has_its_own_machine_code() {
    assert_eq!(
        KnowledgeBankError::NotFound("doc".to_owned()).code(),
        "NOT_FOUND"
    );
    assert_eq!(
        KnowledgeBankError::Forbidden("admin only".to_owned()).code(),
        "FORBIDDEN"
    );
    assert_eq!(
        KnowledgeBankError::Internal("boom".to_owned()).code(),
        "INTERNAL_ERROR"
    );
    let serde_err: KnowledgeBankError = serde_json::from_str::<u32>("nope")
        .expect_err("invalid JSON")
        .into();
    assert_eq!(serde_err.code(), "SERIALIZATION_ERROR");
}

#[test]
fn statuses_separate_client_faults_from_server_faults() {
    assert_eq!(
        KnowledgeBankError::NotFound("doc".to_owned()).status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        KnowledgeBankError::Forbidden("admin only".to_owned()).status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        KnowledgeBankError::Internal("boom".to_owned()).status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    let serde_err: KnowledgeBankError = serde_json::from_str::<u32>("nope")
        .expect_err("invalid JSON")
        .into();
    assert_eq!(serde_err.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[test]
fn no_variant_is_retryable() {
    let variants = [
        KnowledgeBankError::NotFound("doc".to_owned()),
        KnowledgeBankError::Forbidden("admin only".to_owned()),
        KnowledgeBankError::Internal("boom".to_owned()),
    ];
    for variant in variants {
        assert!(!variant.is_retryable(), "{variant} must not be retryable");
    }
}

#[test]
fn display_prefixes_the_variant_so_logs_stay_greppable() {
    assert_eq!(
        KnowledgeBankError::NotFound("doc-1".to_owned()).to_string(),
        "Not found: doc-1"
    );
    assert_eq!(
        KnowledgeBankError::Forbidden("admin only".to_owned()).to_string(),
        "Forbidden: admin only"
    );
}
