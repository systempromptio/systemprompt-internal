//! The admin server (`systemprompt-mcp-agent`), re-enabled: it offers the CLI
//! passthrough plus the three approval tools behind the governance-approvals
//! dashboard, and a user-role bearer is refused before any command runs.

use crate::harness::mcp;
use crate::harness::stack::Stack;

#[tokio::test]
async fn the_admin_server_offers_the_cli_and_approval_tools_and_refuses_a_user() {
    let Some(stack) = Stack::create().await else {
        return;
    };
    let manifest = stack.manifest(&stack.admin_token).await;
    let servers: Vec<&str> = manifest["managed_mcp_servers"]
        .as_array()
        .expect("servers")
        .iter()
        .filter_map(|s| s["name"].as_str())
        .collect();
    assert!(
        servers.contains(&"systemprompt"),
        "the server is enabled and granted to admins, so the admin manifest lists it: {servers:?}"
    );

    let Some(server) = mcp::spawn_systemprompt_mcp().await else {
        stack.db.cleanup().await;
        return;
    };

    let tools = mcp::list_tools(server.port, &stack.admin_token)
        .await
        .expect("tools/list as admin");
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert_eq!(
        names,
        [
            "systemprompt",
            "approval_list",
            "approval_decide",
            "approval_history"
        ],
        "the CLI passthrough plus the three approval tools the \
         governance-approvals dashboard allowlists by name: {names:?}"
    );

    let denied = mcp::call_tool(
        server.port,
        &stack.user_token,
        "systemprompt",
        serde_json::json!({ "command": "core skills list" }),
    )
    .await;
    assert!(
        denied.is_err(),
        "a user-role caller must be refused before the CLI runs: {denied:?}"
    );

    drop(server);
    stack.db.cleanup().await;
}
