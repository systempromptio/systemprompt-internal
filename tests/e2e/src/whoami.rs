//! `comms_whoami` over the real MCP wire: the report is the caller's own, the
//! grants come from the same resolver the bridge manifest uses, and the Odoo
//! key never leaves the database.

use systemprompt_mcp_comms::whoami::{GrantSource, WhoamiReport};

use crate::harness::mcp;
use crate::harness::stack::Stack;

const USER_EMAIL: &str = "ed+notadmin@systemprompt.io";

fn ids(entities: &[systemprompt_mcp_comms::whoami::GrantedEntity]) -> Vec<&str> {
    entities.iter().map(|e| e.id.as_str()).collect()
}

async fn whoami(port: u16, bearer: &str) -> WhoamiReport {
    let text = mcp::call_tool(port, bearer, "comms_whoami", serde_json::json!({}))
        .await
        .expect("comms_whoami succeeds");
    // The tool answers with a text artifact whose body is the report JSON;
    // hosts without the UI extension see that body verbatim.
    let body = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| v.get("content").and_then(|c| c.as_str()).map(str::to_owned))
        .unwrap_or(text);
    serde_json::from_str(&body).unwrap_or_else(|e| panic!("whoami body is a report: {e}\n{body}"))
}

#[tokio::test]
async fn whoami_reports_the_caller_and_the_grants_their_role_resolves_to() {
    let Some(stack) = Stack::create().await else {
        return;
    };
    let Some(server) = mcp::spawn_comms_mcp().await else {
        stack.db.cleanup().await;
        return;
    };

    let tools = mcp::list_tools(server.port, &stack.user_token)
        .await
        .expect("tools/list");
    let tool = tools
        .iter()
        .find(|t| t.name == "comms_whoami")
        .expect("comms_whoami is offered");
    assert_eq!(
        tool.annotations.as_ref().and_then(|a| a.read_only_hint),
        Some(true),
        "the dashboard caching contract needs readOnlyHint on comms_whoami"
    );

    let user = whoami(server.port, &stack.user_token).await;
    assert_eq!(
        user.user.email, USER_EMAIL,
        "the report is the caller's own"
    );
    assert!(user.user.roles.contains(&"user".to_owned()));
    assert!(!user.user.roles.contains(&"admin".to_owned()));
    assert!(
        !user.odoo.linked,
        "the seeded e2e user has no Odoo link yet"
    );

    let plugins = ids(&user.grants.plugins);
    for plugin in [
        "systemprompt-commons",
        "systemprompt-user",
        "systemprompt-demo",
    ] {
        assert!(
            plugins.contains(&plugin),
            "{plugin} is granted to users: {plugins:?}"
        );
    }
    assert!(
        !plugins.contains(&"systemprompt-admin"),
        "the admin plugin must not resolve for a user: {plugins:?}"
    );
    let skills = ids(&user.grants.skills);
    assert!(skills.contains(&"systemprompt_setup_cowork"), "{skills:?}");
    assert!(
        !skills.contains(&"admin_user_report"),
        "a ruleless admin skill is closed by its plugin: {skills:?}"
    );
    let setup = user
        .grants
        .skills
        .iter()
        .find(|g| g.id == "systemprompt_setup_cowork")
        .expect("setup skill granted");
    assert_eq!(
        setup.via,
        GrantSource::Plugin("systemprompt-commons".to_owned()),
        "a ruleless skill is attributed to the plugin that admitted it"
    );
    let servers = ids(&user.grants.mcp_servers);
    assert!(servers.contains(&"comms"));
    assert!(!servers.contains(&"systemprompt"), "{servers:?}");

    let admin = whoami(server.port, &stack.admin_token).await;
    assert!(admin.user.roles.contains(&"admin".to_owned()));
    let admin_plugins = ids(&admin.grants.plugins);
    assert!(
        admin_plugins.contains(&"systemprompt-admin"),
        "{admin_plugins:?}"
    );
    assert!(
        ids(&admin.grants.skills).contains(&"admin_user_report"),
        "an admin resolves the admin skills through the plugin rule"
    );
    assert!(ids(&admin.grants.mcp_servers).contains(&"systemprompt"));

    let raw = serde_json::to_string(&admin).expect("serialises");
    assert!(
        !raw.contains("api_key") && !raw.contains("odoo_api_key"),
        "the Odoo key never appears in a whoami report: {raw}"
    );

    drop(server);
    stack.db.cleanup().await;
}
