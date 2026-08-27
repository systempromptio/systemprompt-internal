//! Per-role bridge manifest content — the skills coverage.
//!
//! The admin/salesperson demo depends on one property: the same gateway, the
//! same marketplace, but a different manifest per role. roles.yaml grants the
//! `systemprompt` MCP server, the `systemprompt-admin` plugin, and the admin
//! skills to `[admin]` with `default_included: false`; everything else rides
//! the marketplace's `[user]` grant. These tests pin that split at the wire,
//! against the shipped `services/` tree, through the real inventory-registered
//! marketplace filter.

use std::collections::BTreeSet;

use crate::harness::stack::Stack;

fn ids(manifest: &serde_json::Value, key: &str) -> BTreeSet<String> {
    manifest[key]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|e| {
                    e.get("id")
                        .or_else(|| e.get("name"))
                        .and_then(|v| v.as_str())
                        .map(str::to_owned)
                })
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn an_admin_manifest_carries_the_admin_surface_and_a_users_does_not() {
    let Some(stack) = Stack::create().await else {
        return;
    };

    let admin = stack.manifest(&stack.admin_token).await;
    let user = stack.manifest(&stack.user_token).await;

    let admin_skills = ids(&admin, "skills");
    let user_skills = ids(&user, "skills");
    let admin_only: BTreeSet<_> = admin_skills.difference(&user_skills).collect();

    assert!(
        !user_skills.is_empty(),
        "a plain user still gets the marketplace's default skills: {user:#}"
    );
    assert!(
        admin_only.contains(&"admin_workspace_setup_cowork".to_owned()),
        "the admin workspace setup skill is the admin-gated canary; admin-only set was \
         {admin_only:?}"
    );
    assert!(
        !user_skills.contains("admin_workspace_setup_cowork"),
        "a salesperson must not be offered the admin workspace skill"
    );

    // The enterprise demo (DEMO.md) ships as its own systemprompt-demo plugin
    // and splits its five skills across the role boundary: steps 1–3 ride the
    // marketplace's [user] grant, steps 4–5 are admin-gated in roles.yaml.
    for demo_skill in [
        "demo_lead_triage",
        "demo_account_360",
        "demo_followup_orchestrator",
    ] {
        assert!(
            user_skills.contains(demo_skill),
            "demo steps 1-3 ride the [user] grant; user skills: {user_skills:?}"
        );
    }
    for demo_skill in ["demo_governed_operations", "demo_command_center"] {
        assert!(
            admin_only.contains(&demo_skill.to_owned()),
            "demo steps 4-5 are admin-only; admin-only set: {admin_only:?}"
        );
    }

    let admin_plugins = ids(&admin, "plugins");
    let user_plugins = ids(&user, "plugins");
    assert!(
        admin_plugins.contains("systemprompt-admin"),
        "roles.yaml grants systemprompt-admin to [admin]; admin plugins: {admin_plugins:?}"
    );
    assert!(
        !user_plugins.contains("systemprompt-admin"),
        "the admin plugin must not reach a user manifest: {user_plugins:?}"
    );

    let admin_mcp = ids(&admin, "managed_mcp_servers");
    let user_mcp = ids(&user, "managed_mcp_servers");
    assert!(
        user_mcp.contains("odoo"),
        "the odoo MCP server is granted to [user]: {user_mcp:?}"
    );
    assert!(
        !user_mcp.contains("systemprompt"),
        "the admin-gated systemprompt MCP server must not reach a user: {user_mcp:?}"
    );
    // The systemprompt server is disabled in services/mcp/systemprompt.yaml,
    // so even the admin manifest may omit it; the load-bearing assertion is
    // the user-side exclusion above, plus admin ⊇ user.
    assert!(
        admin_mcp.is_superset(&user_mcp),
        "an admin never sees fewer servers than a user: admin {admin_mcp:?} vs user {user_mcp:?}"
    );

    stack.db.cleanup().await;
}

#[tokio::test]
async fn a_manifest_names_the_user_it_was_assembled_for() {
    let Some(stack) = Stack::create().await else {
        return;
    };

    let manifest = stack.manifest(&stack.user_token).await;
    let email = manifest["user"]["email"].as_str().unwrap_or_default();
    assert_eq!(
        email, "e2e-user@e2e.test",
        "the manifest is per-user, not a shared document: {manifest:#}"
    );

    stack.db.cleanup().await;
}
