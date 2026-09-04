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
    for plugin_id in ["systemprompt-business", "systemprompt-demo"] {
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

    let business = bundle_paths(&manifest, "systemprompt-business");
    assert!(
        !business.iter().any(|p| p.contains("systemprompt-setup")),
        "business ships no setup skill — installing dashboards is admin-only: {business:?}"
    );
    let skills: Vec<&str> = manifest["skills"]
        .as_array()
        .expect("skills present")
        .iter()
        .filter_map(|s| s["id"].as_str())
        .collect();
    // This is the user manifest: it carries no setup skill of any kind.
    // Installing dashboards is admin-only, and the one installer
    // (systemprompt_setup_admin) is closed to this role by the admin plugin's
    // grant — manifest_roles.rs pins both halves of that.
    assert!(
        !skills.iter().any(|id| id.starts_with("systemprompt_setup")),
        "a user manifest carries no setup skill — installing is admin-only: {skills:?}"
    );
    // Retired ids must not come back through a stale bundle or a re-added
    // config: the shared setup skill and its two host bodies, the first demo
    // plugin's five narrations, the CLI manuals (inspect, report), the
    // user-plugin skills that show_activity and update_leads absorbed, and
    // the three narrow demo skills plus brand/company_context/my_workspace/
    // lead_factsheet/governance_readback that manage_leads, send_email and
    // demonstrate_governance absorbed in turn. send_email itself is now
    // retired outright — the `email` MCP server it depended on was removed.
    for retired in [
        "systemprompt_setup",
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
        "brand",
        "company_context",
        "show_activity",
        "update_leads",
        "my_workspace",
        "lead_factsheet",
        "demo_approval_hold",
        "demo_blocked_tool",
        "demo_secret_refusal",
        "governance_readback",
        "send_email",
        "manage_platform",
    ] {
        assert!(
            !skills.contains(&retired),
            "{retired} was consolidated away and must not reach a manifest: {skills:?}"
        );
    }

    // The business bundle is user-scoped, so its allowlists are pinned off the
    // user token: a user must be able to pull it, and each dashboard must carry
    // exactly the tools its page calls.
    let (_, body) = stack
        .send(
            "GET",
            "/v1/bridge/plugins/systemprompt-business/artifacts/manifest.json",
            Some(&stack.user_token),
            None,
        )
        .await;
    let business_bundle: serde_json::Value =
        serde_json::from_str(&body).expect("business manifest.json parses");
    let business_tools = |id: &str| -> serde_json::Value {
        business_bundle["artifacts"]
            .as_array()
            .expect("artifact records")
            .iter()
            .find(|a| a["id"] == id)
            .unwrap_or_else(|| panic!("{id} bundled in systemprompt-business"))["mcpTools"]
            .clone()
    };
    // Both dashboards write as well as read, so the allowlist is now load
    // bearing twice over: short it and the page still renders and still loads
    // data, but every button fails at the click. The order is the order the
    // config declares — reads, then writes.
    assert_eq!(
        business_tools("my-day"),
        serde_json::json!([
            "mcp__odoo__business_overview_data",
            "mcp__odoo__activity_list",
            "mcp__odoo__task_list",
            "mcp__odoo__note_search",
            "mcp__odoo__crm_stage_list",
            "mcp__odoo__activity_complete",
            "mcp__odoo__task_update",
            "mcp__odoo__crm_lead_update",
            "mcp__odoo__note_add"
        ]),
        "my-day reads the briefing, activities, tasks, notes and stages, and writes back through \
         the tick, the star, the stage menu and the note button"
    );
    assert_eq!(
        business_tools("sales-pipeline"),
        serde_json::json!([
            "mcp__odoo__crm_lead_search",
            "mcp__odoo__crm_stage_list",
            "mcp__odoo__note_list",
            "mcp__odoo__crm_lead_update",
            "mcp__odoo__crm_lead_mark_won",
            "mcp__odoo__crm_lead_mark_lost",
            "mcp__odoo__note_add",
            "mcp__odoo__activity_create"
        ]),
        "sales-pipeline reads leads, stages and chatter, and writes back through the stage menu, \
         Won/Lost, the note button and the follow-up button"
    );
    // crm_lead_delete is the server's one Destructive tool. A dashboard click
    // must not be able to unlink a record, so no bundled artifact may carry it.
    for id in ["my-day", "sales-pipeline"] {
        let tools = business_tools(id).to_string();
        assert!(
            !tools.contains("crm_lead_delete"),
            "{id} must never allow the destructive crm_lead_delete: {tools}"
        );
    }

    stack.db.cleanup().await;
}

#[tokio::test]
async fn the_admin_bundle_is_served_to_admins_and_refused_to_users() {
    let Some(stack) = Stack::create().await else {
        return;
    };

    let admin = stack.manifest(&stack.admin_token).await;
    let paths = bundle_paths(&admin, "systemprompt-admin");
    for id in ["admin-activity-requests", "admin-usage-costs"] {
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
