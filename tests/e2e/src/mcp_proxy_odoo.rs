//! The MCP wire, end to end: real binary, real protocol, real identity.
//!
//! This is the flow Cowork's Recent Activity dashboard runs: a signed-in
//! Odoo user's chatter tools, executed as that user via their auto-linked
//! credential. The note_search "%" round-trip is the regression that
//! motivated this suite — a wildcard query must return the notes the same
//! user just posted, not an empty feed.

use axum::http::StatusCode;

use crate::artifact_gallery::BRAND_ACCENT;
use crate::harness::mcp;
use crate::harness::odoo_mock::GOOD_CREDENTIAL;
use crate::harness::stack::Stack;

const LOGIN: &str = "e2e-notes@systemprompt.local";

// Why: the note body is the one field these two tools exist to return, and it
// is reached the way a dashboard reaches it — `items`, then `body`. A row that
// does not deserialise is simply not a match, so a shape change fails the
// assertion rather than passing on a lucky substring elsewhere in the JSON.
fn note_bodies(structured: Option<&serde_json::Value>) -> impl Iterator<Item = &str> {
    structured
        .and_then(|v| v.get("items"))
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|row| row.get("body").and_then(serde_json::Value::as_str))
}


#[tokio::test]
async fn a_signed_in_user_posts_and_finds_notes_over_the_mcp_wire() {
    let Some(stack) = Stack::create().await else {
        return;
    };
    // Sign in first: JIT-provisions the platform user AND auto-links the Odoo
    // credential the MCP tools execute with.
    let (status, body) = stack.odoo_login(LOGIN, GOOD_CREDENTIAL).await;
    assert_eq!(status, StatusCode::OK, "odoo sign-in: {body}");
    let bearer = stack.token_for_email(LOGIN).await;

    let Some(server) = mcp::spawn_odoo_mcp(&stack.odoo.url()).await else {
        stack.db.cleanup().await;
        return;
    };

    let added = mcp::call_tool(
        server.port,
        &bearer,
        "note_add",
        serde_json::json!({ "model": "crm.lead", "res_id": 1, "body": "E2E wildcard note" }),
    )
    .await
    .expect("note_add succeeds");
    assert!(
        added.contains("Note posted"),
        "note_add confirms the post: {added}"
    );

    // note_list and note_search answer with typed tables: the body lives in a
    // row under `items`, and the text block carries only the count. Asserting
    // on the text would be reading the summary and calling it the data — the
    // exact coupling these tools were converted to remove.
    let url = format!("http://127.0.0.1:{}/mcp", server.port);
    let (listed, listed_rows) = mcp::call_tool_full(
        &url,
        &bearer,
        "note_list",
        serde_json::json!({ "model": "crm.lead", "res_id": 1 }),
    )
    .await
    .expect("note_list succeeds");
    assert!(
        note_bodies(listed_rows.as_ref()).any(|b| b.contains("E2E wildcard note")),
        "the thread shows the note in its rows: {listed} / {listed_rows:?}"
    );

    let (searched, searched_rows) = mcp::call_tool_full(
        &url,
        &bearer,
        "note_search",
        serde_json::json!({ "query": "%", "limit": 50 }),
    )
    .await
    .expect("note_search succeeds");
    assert!(
        note_bodies(searched_rows.as_ref()).any(|b| b.contains("E2E wildcard note")),
        "a wildcard search is match-all, not a literal percent hunt: {searched} / \
         {searched_rows:?}"
    );

    // The lead dashboards' contract: crm_lead_search answers with a typed
    // table — columns named in Odoo's own fields, rows under `items` — not
    // markdown for the dashboards to regex apart.
    let (summary, structured) = mcp::call_tool_full(
        &format!("http://127.0.0.1:{}/mcp", server.port),
        &bearer,
        "crm_lead_search",
        serde_json::json!({ "limit": 10 }),
    )
    .await
    .expect("crm_lead_search succeeds");
    assert!(summary.contains("lead(s) matched"), "summary: {summary}");
    let table = structured.expect("structured content present");
    let first = &table["items"][0];
    assert_eq!(
        first["name"], "E2E Table Lead",
        "typed rows under items: {table:#}"
    );
    assert_eq!(first["stage_id"], "New", "many2one collapsed to its name");
    assert_eq!(first["email_from"], "buyer@acme.test");
    assert_eq!(first["expected_revenue"], 1250.5);

    drop(server);
    stack.db.cleanup().await;
}

// The whole chain in one assertion: real binary, real protocol, real identity,
// and the artifact HTML exactly as Cowork receives it — theme included. The
// gallery's other twelve entries are rendered in-process; this is the one that
// proves the branding survives the process boundary into a shipped binary.
#[tokio::test]
async fn the_crm_table_arrives_as_a_branded_ui_resource() {
    let Some(stack) = Stack::create().await else {
        return;
    };
    let (status, body) = stack.odoo_login(LOGIN, GOOD_CREDENTIAL).await;
    assert_eq!(status, StatusCode::OK, "odoo sign-in: {body}");
    let bearer = stack.token_for_email(LOGIN).await;

    let Some(server) = mcp::spawn_odoo_mcp(&stack.odoo.url()).await else {
        stack.db.cleanup().await;
        return;
    };

    let ui = mcp::call_tool_resource(
        &format!("http://127.0.0.1:{}/mcp", server.port),
        &bearer,
        "crm_lead_search",
        serde_json::json!({ "limit": 10 }),
    )
    .await
    .expect("crm_lead_search returns an embedded UI resource");

    assert!(
        ui.uri.starts_with("ui://"),
        "the host dispatches on this scheme: {}",
        ui.uri
    );
    assert_eq!(
        ui.mime_type.as_deref(),
        Some("text/html;profile=mcp-app"),
        "the mime is what tells Cowork to mount this as an app, not show it as text"
    );
    assert!(
        ui.html.contains(BRAND_ACCENT),
        "the shipped binary renders unbranded — the ArtifactTheme registration \
         in systemprompt-mcp-shared did not survive linking"
    );
    crate::artifact_gallery::write_gallery_entry(
        &crate::artifact_gallery::gallery_dir(),
        "wire-crm-lead-search",
        &ui.html,
    );
    // The table is rendered client-side from this column spec, so there is no
    // `<th>` in the served HTML to read. Assert the mapping itself: the Odoo
    // field name survives as the data key while the header is the human label.
    // Why not a bare `!html.contains("STAGE_ID")` — core's own stylesheet
    // carries a comment naming that string as the bug it prevents, so the
    // negative matched prose and failed on correct output.
    for (key, header) in [
        ("stage_id", "Stage"),
        ("user_id", "Salesperson"),
        ("expected_revenue", "Expected revenue"),
    ] {
        assert!(
            ui.html
                .contains(&format!(r#""key":"{key}","header":"{header}""#)),
            "column {key} must be headed {header:?}, not its raw field name"
        );
    }

    drop(server);
    stack.db.cleanup().await;
}

#[tokio::test]
async fn the_gateway_proxies_the_odoo_mcp_route() {
    let Some(stack) = Stack::create().await else {
        return;
    };

    // No subprocess is running here; the property is that the gateway OWNS the
    // route (auth challenge or upstream failure), rather than 404ing — a 404
    // means the proxy mount for services/mcp/odoo.yaml is gone.
    let (status, body) = stack.send("POST", "/api/v1/mcp/odoo/mcp", None, None).await;
    assert_ne!(
        status,
        StatusCode::NOT_FOUND,
        "the MCP proxy route must be mounted: {body}"
    );

    stack.db.cleanup().await;
}
