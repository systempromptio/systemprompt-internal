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
    for plugin_id in ["systemprompt-commons", "systemprompt-user"] {
        let paths = bundle_paths(&manifest, plugin_id);
        assert!(
            paths.contains(&"artifacts/manifest.json"),
            "{plugin_id} ships its artifact install manifest; bundle had {paths:?}"
        );
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
        commons
            .iter()
            .any(|p| p.contains("systemprompt-setup-cowork")),
        "the one setup skill ships in commons: {commons:?}"
    );
    assert!(
        commons.contains(&"artifacts/whoami.html"),
        "the Who Am I panel ships in commons: {commons:?}"
    );
    // Host targeting: the bundle is the Claude-family skill surface, so the
    // codex-only setup skill must not reach the Cowork picker — while the
    // manifest still carries it (Codex's own emitter reads manifest.skills).
    assert!(
        !commons
            .iter()
            .any(|p| p.contains("systemprompt-setup-codex")),
        "the codex setup skill leaked into the Claude bundle: {commons:?}"
    );
    let skills: Vec<&str> = manifest["skills"]
        .as_array()
        .expect("skills present")
        .iter()
        .filter_map(|s| s["id"].as_str())
        .collect();
    assert!(skills.contains(&"systemprompt_setup_codex"));
    assert!(
        skills.contains(&"systemprompt_setup"),
        "the router is the one name every role types: {skills:?}"
    );

    let (_, body) = stack
        .send(
            "GET",
            "/v1/bridge/plugins/systemprompt-user/artifacts/manifest.json",
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
