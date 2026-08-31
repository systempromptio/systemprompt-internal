//! Per-role bridge manifest content — the plugin-scope proof.
//!
//! The admin/salesperson demo depends on one property: the same gateway, the
//! same marketplace, but a different manifest per role. roles.yaml declares
//! one `entity_type: plugin` rule per plugin — commons/user/demo to `[user]`,
//! admin to `[admin]` with `default_included: false` — and NO per-skill
//! rules: every skill and artifact inherits its plugin. These tests pin that
//! cascade at the wire, against the shipped `services/` tree, through the
//! real inventory-registered marketplace filter.

use std::collections::BTreeSet;

use crate::harness::stack::Stack;

const USER_PLUGINS: &[&str] = &[
    "systemprompt-commons",
    "systemprompt-user",
    "systemprompt-demo",
];
const ADMIN_PLUGINS: &[&str] = &["systemprompt-admin"];
// todo-bulletin and knowledge-feed are shelved with the knowledge surface
// (each config.yaml carries enabled: false), so they are no longer named by any
// plugin and must not reach a manifest.
const USER_ARTIFACTS: &[&str] = &[
    "business-overview",
    "leads-inbound-prospects",
    "pipeline-open-deals",
    "recent-activity",
];
const ADMIN_ARTIFACTS: &[&str] = &[
    "admin-users-directory",
    "admin-activity-requests",
    "admin-usage-costs",
];

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

fn plugin_skill_ids(plugin_id: &str) -> BTreeSet<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root");
    let text =
        std::fs::read_to_string(root.join(format!("services/plugins/{plugin_id}/config.yaml")))
            .expect("plugin config");
    let doc: serde_yaml::Value = serde_yaml::from_str(&text).expect("plugin yaml");
    doc["plugin"]["skills"]["include"]
        .as_sequence()
        .expect("explicit skills include")
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect()
}

#[tokio::test]
async fn an_admin_manifest_carries_the_admin_surface_and_a_users_does_not() {
    let Some(stack) = Stack::create().await else {
        return;
    };

    let admin = stack.manifest(&stack.admin_token).await;
    let user = stack.manifest(&stack.user_token).await;

    let admin_plugins = ids(&admin, "plugins");
    let user_plugins = ids(&user, "plugins");
    for plugin in USER_PLUGINS {
        assert!(
            user_plugins.contains(*plugin),
            "user-scoped plugin {plugin} rides the [user] grant; user plugins: {user_plugins:?}"
        );
    }
    for plugin in ADMIN_PLUGINS {
        assert!(
            admin_plugins.contains(*plugin),
            "roles.yaml grants {plugin} to [admin]; admin plugins: {admin_plugins:?}"
        );
        assert!(
            !user_plugins.contains(*plugin),
            "the admin plugin must not reach a user manifest: {user_plugins:?}"
        );
    }
    assert!(
        admin_plugins.is_superset(&user_plugins),
        "admins hold the user role, so an admin never sees fewer plugins: admin \
         {admin_plugins:?} vs user {user_plugins:?}"
    );

    let admin_skills = ids(&admin, "skills");
    let user_skills = ids(&user, "skills");
    for plugin in USER_PLUGINS {
        for skill in plugin_skill_ids(plugin) {
            assert!(
                user_skills.contains(&skill),
                "{skill} ships in user-scoped {plugin} and carries no rule of its own, so it \
                 inherits the plugin grant; user skills: {user_skills:?}"
            );
        }
    }
    for plugin in ADMIN_PLUGINS {
        for skill in plugin_skill_ids(plugin) {
            assert!(
                admin_skills.contains(&skill),
                "{skill} ships in {plugin}; admin skills: {admin_skills:?}"
            );
            assert!(
                !user_skills.contains(&skill),
                "{skill} carries no rule of its own and ships only in admin-scoped {plugin}; \
                 the plugin rule must close it to users — this is the cascade proof"
            );
        }
    }

    let admin_artifacts = ids(&admin, "artifacts");
    let user_artifacts = ids(&user, "artifacts");
    for artifact in USER_ARTIFACTS {
        assert!(
            user_artifacts.contains(*artifact),
            "{artifact} ships in a user-scoped plugin; user artifacts: {user_artifacts:?}"
        );
    }
    for artifact in ADMIN_ARTIFACTS {
        assert!(
            admin_artifacts.contains(*artifact),
            "{artifact} ships in systemprompt-admin; admin artifacts: {admin_artifacts:?}"
        );
        assert!(
            !user_artifacts.contains(*artifact),
            "an admin dashboard must never reach a user: {user_artifacts:?}"
        );
    }
    assert!(admin_artifacts.is_superset(&user_artifacts));

    let admin_mcp = ids(&admin, "managed_mcp_servers");
    let user_mcp = ids(&user, "managed_mcp_servers");
    // knowledge-bank now reaches a user only through the demo plugin.
    for server in ["odoo", "email"] {
        assert!(
            user_mcp.contains(server),
            "{server} is granted to [user]: {user_mcp:?}"
        );
    }
    assert!(
        admin_mcp.contains("systemprompt"),
        "the admin CLI server is enabled and granted to [admin]: {admin_mcp:?}"
    );
    assert!(
        !user_mcp.contains("systemprompt"),
        "the admin-gated systemprompt MCP server must not reach a user: {user_mcp:?}"
    );

    stack.db.cleanup().await;
}

#[tokio::test]
async fn a_ruleless_skill_in_an_admin_plugin_never_reaches_a_user() {
    let Some(stack) = Stack::create().await else {
        return;
    };

    let user = stack.manifest(&stack.user_token).await;
    let user_skills = ids(&user, "skills");
    let leaked: Vec<String> = plugin_skill_ids("systemprompt-admin")
        .into_iter()
        .filter(|s| user_skills.contains(s))
        .collect();
    assert!(
        leaked.is_empty(),
        "admin skills reached a user manifest: {leaked:?} — the plugin-level cascade is not \
         closing them (is the server built against a core with the plugin parent chain?)"
    );

    stack.db.cleanup().await;
}

#[tokio::test]
async fn setup_is_split_by_role_and_both_roles_keep_the_shared_front_door() {
    let Some(stack) = Stack::create().await else {
        return;
    };

    let user_skills = ids(&stack.manifest(&stack.user_token).await, "skills");
    let admin_skills = ids(&stack.manifest(&stack.admin_token).await, "skills");

    // The router ships in systemprompt-commons, which every role holds.
    for (role, skills) in [("user", &user_skills), ("admin", &admin_skills)] {
        assert!(
            skills.contains("systemprompt_setup"),
            "{role} lost the shared setup front door: {skills:?}"
        );
    }

    // The control-plane installer ships only in the admin plugin, so the split
    // is enforced by the grant — not by a branch inside a shared skill.
    assert!(
        admin_skills.contains("systemprompt_setup_admin"),
        "admin lost the control-plane setup: {admin_skills:?}"
    );
    assert!(
        !user_skills.contains("systemprompt_setup_admin"),
        "the control-plane setup reached a user manifest: {user_skills:?}"
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
