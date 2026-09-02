//! Skills and artifacts delivery: the bundle a Cowork install actually pulls.
//!
//! The manifest names every file of every plugin with a hash; the bridge then
//! fetches each through `/v1/bridge/plugins/{id}/{*path}` and verifies it.
//! Every plugin bundle lays its dashboards out as `artifacts/manifest.json`
//! plus one `artifacts/<id>.html` per record, straight from
//! `services/artifacts/`; the one setup skill installs whatever bundles the
//! bridge mounted. These tests walk that path for both roles.

use axum::http::StatusCode;

use crate::harness::stack::Stack;

// The artifact ids a plugin declares, read from the shipped config rather than
// duplicated here — a plugin that ships no dashboards (commons) must not be
// asserted to carry an artifact install manifest.
fn plugin_artifact_ids(plugin_id: &str) -> Vec<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root");
    let text =
        std::fs::read_to_string(root.join(format!("services/plugins/{plugin_id}/config.yaml")))
            .expect("plugin config");
    let doc: serde_yaml::Value = serde_yaml::from_str(&text).expect("plugin yaml");
    doc["plugin"]["artifacts"]["include"]
        .as_sequence()
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn bundle_paths<'a>(manifest: &'a serde_json::Value, plugin_id: &str) -> Vec<&'a str> {
    manifest["plugins"]
        .as_array()
        .expect("plugins present")
        .iter()
        .find(|p| p["id"] == plugin_id)
        .unwrap_or_else(|| panic!("{plugin_id} is in the manifest"))["files"]
        .as_array()
        .expect("bundle files listed")
        .iter()
        .filter_map(|f| f["path"].as_str())
        .collect()
}

#[tokio::test]
async fn every_manifest_named_bundle_file_is_fetchable_and_dashboards_ship_with_their_pages() {
    let Some(stack) = Stack::create().await else {
        return;
    };

    let manifest = stack.manifest(&stack.user_token).await;
    for plugin_id in [
        "systemprompt-commons",
        "systemprompt-demo",
        "systemprompt-workspace",
    ] {
        let paths = bundle_paths(&manifest, plugin_id);
        let declared = plugin_artifact_ids(plugin_id);
        if declared.is_empty() {
            assert!(
                !paths.contains(&"artifacts/manifest.json"),
                "{plugin_id} declares no artifacts, so it must ship no install \
                 manifest; bundle had {paths:?}"
            );
        } else {
            assert!(
                paths.contains(&"artifacts/manifest.json"),
                "{plugin_id} declares {declared:?} so it ships an artifact install \
                 manifest; bundle had {paths:?}"
            );
        }
        for path in &paths {
            let (status, body) = stack
                .send(
                    "GET",
                    &format!("/v1/bridge/plugins/{plugin_id}/{path}"),
                    Some(&stack.user_token),
                    None,
                )
                .await;
            assert_eq!(status, StatusCode::OK, "{path} must be fetchable: {body}");
            assert!(!body.is_empty(), "{path} served empty");
        }

        if declared.is_empty() {
            continue;
        }

        let (_, body) = stack
            .send(
                "GET",
                &format!("/v1/bridge/plugins/{plugin_id}/artifacts/manifest.json"),
                Some(&stack.user_token),
                None,
            )
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&body).expect("manifest.json parses");
        for record in parsed["artifacts"].as_array().expect("artifact records") {
            let id = record["id"].as_str().expect("record id");
            assert!(
                paths.contains(&format!("artifacts/{id}.html").as_str()),
                "{plugin_id}: {id} is listed but its page is not beside it: {paths:?}"
            );
            assert!(
                record.get("content").is_none(),
                "the install manifest never embeds HTML"
            );
        }
    }

    let commons = bundle_paths(&manifest, "systemprompt-commons");
    assert!(
        commons.iter().any(|p| p.contains("systemprompt-setup")),
        "the one setup skill ships in commons: {commons:?}"
    );
    let skills: Vec<&str> = manifest["skills"]
        .as_array()
        .expect("skills present")
        .iter()
        .filter_map(|s| s["id"].as_str())
        .collect();
    assert!(
        skills.contains(&"systemprompt_setup"),
        "setup is the one name every role types: {skills:?}"
    );
    // Retired ids must not come back through a stale bundle or a re-added
    // config: the setup router and its two host bodies, the first demo
    // plugin's five narrations, and the CLI manuals (inspect, report) plus the
    // user-plugin skills that show_activity and update_leads absorbed.
    for retired in [
        "systemprompt_setup_cowork",
        "systemprompt_setup_codex",
        "systemprompt_cli",
        "capture_knowledge",
        "demo_lead_triage",
        "demo_account_360",
        "demo_followup_orchestrator",
        "demo_governed_operations",
        "demo_command_center",
        "inspect",
        "report",
        "crm",
        "manage_work",
        "business_overview",
    ] {
        assert!(
            !skills.contains(&retired),
            "{retired} was consolidated away and must not reach a manifest: {skills:?}"
        );
    }

    // The workspace bundle is user-scoped, so its allowlists are pinned off the
    // user token: a user must be able to pull it, and each dashboard must carry
    // exactly the tools its page calls.
    let (_, body) = stack
        .send(
            "GET",
            "/v1/bridge/plugins/systemprompt-workspace/artifacts/manifest.json",
            Some(&stack.user_token),
            None,
        )
        .await;
    let workspace: serde_json::Value =
        serde_json::from_str(&body).expect("workspace manifest.json parses");
    let workspace_tools = |id: &str| -> serde_json::Value {
        workspace["artifacts"]
            .as_array()
            .expect("artifact records")
            .iter()
            .find(|a| a["id"] == id)
            .unwrap_or_else(|| panic!("{id} bundled in systemprompt-workspace"))["mcpTools"]
            .clone()
    };
    assert_eq!(
        workspace_tools("recent-activity"),
        serde_json::json!(["mcp__odoo__note_search"]),
        "recent-activity's allowlist is note_search and nothing else — the cross-wire regression"
    );
    assert_eq!(
        workspace_tools("upcoming-deals"),
        serde_json::json!(["mcp__odoo__crm_lead_search"]),
        "upcoming-deals is a read-only view over crm_lead_search"
    );
    assert_eq!(
        workspace_tools("pipeline-open-deals"),
        serde_json::json!(["mcp__odoo__crm_lead_search"]),
        "pipeline-open-deals is a read-only view over crm_lead_search"
    );
    assert_eq!(
        workspace_tools("todo-bulletin"),
        serde_json::json!([
            "mcp__odoo__activity_list",
            "mcp__odoo__task_list",
            "mcp__odoo__activity_complete"
        ]),
        "todo-bulletin reads two lists and carries exactly one write: the tick"
    );

    let (_, body) = stack
        .send(
            "GET",
            "/v1/bridge/plugins/systemprompt-admin/artifacts/manifest.json",
            Some(&stack.admin_token),
            None,
        )
        .await;
    let parsed: serde_json::Value = serde_json::from_str(&body).expect("manifest.json parses");

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

#[tokio::test]
async fn the_admin_bundle_is_served_to_admins_and_refused_to_users() {
    let Some(stack) = Stack::create().await else {
        return;
    };

    let admin = stack.manifest(&stack.admin_token).await;
    let paths = bundle_paths(&admin, "systemprompt-admin");
    for id in [
        "admin-users-directory",
        "admin-activity-requests",
        "admin-usage-costs",
        "knowledge-feed",
        "knowledge-approve-ingestion",
    ] {
        assert!(
            paths.contains(&format!("artifacts/{id}.html").as_str()),
            "{id} ships in the admin bundle: {paths:?}"
        );
    }
    let (status, _) = stack
        .send(
            "GET",
            "/v1/bridge/plugins/systemprompt-admin/artifacts/manifest.json",
            Some(&stack.admin_token),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = stack
        .send(
            "GET",
            "/v1/bridge/plugins/systemprompt-admin/artifacts/manifest.json",
            Some(&stack.user_token),
            None,
        )
        .await;
    assert_ne!(
        status,
        StatusCode::OK,
        "a user must not be able to pull the admin bundle by path: {body}"
    );

    stack.db.cleanup().await;
}
