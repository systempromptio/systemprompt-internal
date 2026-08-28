//! Skills and artifacts delivery: the bundle a Cowork install actually pulls.
//!
//! The manifest names every file of every plugin with a hash; the bridge then
//! fetches each through `/v1/bridge/plugins/{id}/{*path}` and verifies it.
//! These tests walk that same path for the CRM plugin's setup-cowork skill —
//! the exact files whose drift caused the Recent Activity allowlist
//! cross-wire — and hold the generated skill assets to their
//! `services/artifacts/` source via the sync script's check mode.

use axum::http::StatusCode;

use crate::harness::stack::Stack;

#[tokio::test]
async fn every_manifest_named_skill_file_is_fetchable() {
    let Some(stack) = Stack::create().await else {
        return;
    };

    let manifest = stack.manifest(&stack.user_token).await;
    let plugins = manifest["plugins"].as_array().expect("plugins present");
    let crm = plugins
        .iter()
        .find(|p| p["id"] == "systemprompt-crm")
        .expect("the CRM plugin rides the [user] marketplace grant");

    let files = crm["files"].as_array().expect("bundle files listed");
    let setup_files: Vec<&str> = files
        .iter()
        .filter_map(|f| f["path"].as_str())
        .filter(|p| p.contains("systemprompt-setup-cowork"))
        .collect();
    assert!(
        setup_files.iter().any(|p| p.ends_with("manifest.json")),
        "the setup skill ships its artifact manifest; bundle had {setup_files:?}"
    );

    for path in &setup_files {
        let (status, body) = stack
            .send(
                "GET",
                &format!("/v1/bridge/plugins/systemprompt-crm/{path}"),
                Some(&stack.user_token),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{path} must be fetchable: {body}");
        assert!(!body.is_empty(), "{path} served empty");
    }

    // Host targeting: the bundle is the Claude-family skill surface, so the
    // codex-only setup skill must not reach the Cowork picker — while the
    // manifest still carries it (Codex's own emitter reads manifest.skills).
    let all_paths: Vec<&str> = files.iter().filter_map(|f| f["path"].as_str()).collect();
    assert!(
        !all_paths
            .iter()
            .any(|p| p.contains("systemprompt-setup-codex")),
        "the codex setup skill leaked into the Claude bundle: {all_paths:?}"
    );
    let skills: Vec<&str> = manifest["skills"]
        .as_array()
        .expect("skills present")
        .iter()
        .filter_map(|s| s["id"].as_str())
        .collect();
    assert!(
        skills.contains(&"systemprompt_setup_codex"),
        "the codex skill still rides the manifest for codex's emitter: {skills:?}"
    );
    assert!(
        !skills.contains(&"systemprompt_setup"),
        "the retired router skill must be gone: {skills:?}"
    );

    let manifest_path = setup_files
        .iter()
        .find(|p| p.ends_with("artifacts/manifest.json"))
        .expect("artifact manifest in the bundle");
    let (_, body) = stack
        .send(
            "GET",
            &format!("/v1/bridge/plugins/systemprompt-crm/{manifest_path}"),
            Some(&stack.user_token),
            None,
        )
        .await;
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("manifest.json parses");
    let recent = parsed["artifacts"]
        .as_array()
        .expect("artifact records")
        .iter()
        .find(|a| a["id"] == "recent-activity")
        .expect("recent-activity bundled");
    assert_eq!(
        recent["mcpTools"],
        serde_json::json!(["mcp__odoo__note_search"]),
        "recent-activity's allowlist is note_search and nothing else — the cross-wire regression"
    );

    // leads-inbound-prospects gained note_list alongside crm_lead_search so a
    // lead row can expand its own chatter inline — pin both, in order, so a
    // future edit can't silently drop the allowlist entry the expand feature
    // depends on (that failure mode is exactly the cross-wire regression
    // above, just for a different dashboard).
    let leads = parsed["artifacts"]
        .as_array()
        .expect("artifact records")
        .iter()
        .find(|a| a["id"] == "leads-inbound-prospects")
        .expect("leads-inbound-prospects bundled");
    assert_eq!(
        leads["mcpTools"],
        serde_json::json!(["mcp__odoo__crm_lead_search", "mcp__odoo__note_list"]),
        "leads-inbound-prospects must allow crm_lead_search and note_list, nothing else"
    );

    stack.db.cleanup().await;
}

#[test]
fn the_skill_artifact_bundles_match_their_source() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root");
    let output = std::process::Command::new("python3")
        .arg(root.join("scripts/sync-cowork-artifacts.py"))
        .arg("--check")
        .output()
        .expect("run the artifact sync check");
    assert!(
        output.status.success(),
        "skill artifact assets drifted from services/artifacts/:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
}
