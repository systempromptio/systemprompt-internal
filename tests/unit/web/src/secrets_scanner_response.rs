// Response-phase coverage for the `secrets` gateway scanner. A credential the
// model emits inside a tool call never appears in a `Text` block, so a scanner
// reading only that variant lets it out; these tests pin the widened surface.

use systemprompt::ai::SafetyScanner;
use systemprompt::models::wire::canonical::{CanonicalContent, CanonicalResponse};
use systemprompt::models::wire::inspect::{SurfaceBudget, string_leaves};
use systemprompt_web_admin::gateway_safety::SecretsScanner;

const TOKEN: &str = "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

fn response(content: Vec<CanonicalContent>) -> CanonicalResponse {
    CanonicalResponse {
        id: "resp-1".to_owned(),
        model: "test-model".to_owned(),
        content,
        ..Default::default()
    }
}

#[tokio::test]
async fn credential_in_a_tool_use_argument_is_flagged() {
    let resp = response(vec![CanonicalContent::ToolUse {
        id: "t1".to_owned(),
        name: "post_webhook".to_owned(),
        input: serde_json::json!({ "headers": { "authorization": TOKEN } }),
        signature: None,
    }]);

    let findings = SecretsScanner::new().scan_response_final(&resp).await;

    assert_eq!(findings.len(), 1, "got {findings:?}");
    assert_eq!(findings[0].category, "secret");
    assert_eq!(findings[0].phase, "response");
}

#[tokio::test]
async fn credential_in_a_tool_result_is_flagged() {
    let resp = response(vec![CanonicalContent::ToolResult {
        tool_use_id: "t1".to_owned(),
        content: vec![CanonicalContent::Text(format!("token={TOKEN}"))],
        is_error: false,
        structured_content: None,
        meta: None,
    }]);

    let findings = SecretsScanner::new().scan_response_final(&resp).await;

    assert_eq!(findings.len(), 1, "got {findings:?}");
    assert_eq!(findings[0].category, "secret");
}

#[tokio::test]
async fn credential_only_in_the_received_surface_is_flagged() {
    let mut resp = response(vec![CanonicalContent::Text("all done".to_owned())]);
    resp.received_surface = string_leaves(
        format!(r#"{{"content":[{{"type":"unmodelled","blob":"{TOKEN}"}}]}}"#).as_bytes(),
        SurfaceBudget::default(),
    );

    let findings = SecretsScanner::new().scan_response_final(&resp).await;

    assert_eq!(findings.len(), 1, "got {findings:?}");
    assert_eq!(findings[0].category, "secret");
}

#[tokio::test]
async fn a_clean_response_yields_nothing() {
    let resp = response(vec![CanonicalContent::Text(
        "here is the summary you asked for".to_owned(),
    )]);

    assert!(SecretsScanner::new().scan_response_final(&resp).await.is_empty());
}
