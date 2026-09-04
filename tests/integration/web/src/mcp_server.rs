//! `SystempromptServer` constructed against a real pool.
//!
//! As with the knowledge bank, construction is what needs a database: the
//! server builds a `ToolUsageRepository` and an `McpArtifactRepository` off the
//! pool before it can serve anything. The advertised identity and capability
//! set are asserted on the constructed server, and the tools it exposes are
//! pinned against `tools::list_tools`.

use std::sync::Arc;

use rmcp::ServerHandler;
use sqlx::PgPool;
use systemprompt::database::Database;
use systemprompt::identifiers::McpServerId;
use systemprompt::security::authz::{DenyAllHook, SharedAuthzHook};
use systemprompt_mcp_agent::{SystempromptServer, tools};

use crate::tempdb::TempDb;

fn hook() -> SharedAuthzHook {
    Arc::new(DenyAllHook::null())
}

fn server(pool: &Arc<PgPool>) -> SystempromptServer {
    let db_pool = Arc::new(Database::from_pools(
        Arc::clone(pool),
        Some(Arc::clone(pool)),
    ));
    SystempromptServer::new(db_pool, McpServerId::new("systemprompt"), hook())
        .expect("construct the systemprompt server against a live pool")
}

#[tokio::test]
async fn new_builds_its_repositories_from_the_pool() {
    let Some(db) = TempDb::create().await else {
        return;
    };

    let built = server(&db.pool);

    assert!(
        built.get_info().instructions.is_some(),
        "a constructed server advertises its usage instructions"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn get_info_advertises_both_tools_and_resources() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let built = server(&db.pool);

    let info = built.get_info();

    assert!(info.capabilities.tools.is_some(), "it serves tools");
    assert!(
        info.capabilities.resources.is_some(),
        "it also serves the artifact-viewer resource, unlike the knowledge bank"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn get_info_carries_the_service_id_branding_and_icons() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let built = server(&db.pool);

    let info = built.get_info();

    assert_eq!(info.server_info.name, "SystemPrompt (systemprompt)");
    assert_eq!(info.server_info.title.as_deref(), Some("SystemPrompt CLI"));
    assert!(
        info.server_info
            .website_url
            .as_ref()
            .is_some_and(|u| u.starts_with("https://")),
        "the advertised website URL is absolute"
    );
    let icons = info
        .server_info
        .icons
        .as_ref()
        .expect("the server advertises icons");
    assert_eq!(icons.len(), 2, "a 32px and a 96px favicon are advertised");
    assert!(
        icons
            .iter()
            .all(|i| i.mime_type.as_deref() == Some("image/png")),
        "both icons declare their MIME type"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn a_different_service_id_only_changes_the_server_name() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let db_pool = Arc::new(Database::from_pools(
        Arc::clone(&db.pool),
        Some(Arc::clone(&db.pool)),
    ));
    let renamed = SystempromptServer::new(db_pool, McpServerId::new("sp-staging"), hook())
        .expect("construct with a different service id");

    let info = renamed.get_info();

    assert_eq!(info.server_info.name, "SystemPrompt (sp-staging)");
    assert_eq!(
        info.instructions,
        server(&db.pool).get_info().instructions,
        "the instructions do not depend on the service id"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn the_server_exposes_the_cli_tool_and_the_three_approval_tools() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let _built = server(&db.pool);

    let listed = tools::list_tools();

    let surface: Vec<(&str, Option<&str>)> = listed
        .iter()
        .map(|t| (t.name.as_ref(), t.title.as_deref()))
        .collect();
    assert_eq!(
        surface,
        vec![
            (tools::TOOL_SYSTEMPROMPT, Some("SystemPrompt CLI")),
            (tools::TOOL_APPROVAL_LIST, Some("Held Calls")),
            (tools::TOOL_APPROVAL_DECIDE, Some("Decide Held Call")),
            (tools::TOOL_APPROVAL_HISTORY, Some("Decided Approvals")),
        ],
        "the CLI tool and the three approval tools are the whole tool surface"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn the_cli_tool_requires_a_command_string() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let _built = server(&db.pool);

    let schema = tools::input_schema();

    assert_eq!(schema.get("type").and_then(|t| t.as_str()), Some("object"));
    assert!(
        schema
            .get("properties")
            .and_then(|p| p.get("command"))
            .is_some(),
        "the tool takes a `command` property: {schema}"
    );
    assert!(
        schema
            .get("required")
            .and_then(|r| r.as_array())
            .is_some_and(|r| r.iter().any(|v| v.as_str() == Some("command"))),
        "`command` is required: {schema}"
    );

    db.cleanup().await;
}

// `list_resources`, `read_resource`, `initialize`, and `call_tool` all take an
// rmcp `RequestContext<RoleServer>`, whose `Peer` only exists once a transport
// is serving; there is no way to build one in a test process. Their bodies
// delegate to core helpers (`build_artifact_viewer_resource`,
// `read_artifact_resource`) that core covers, and `dispatch_tool` is private
// to the crate.
