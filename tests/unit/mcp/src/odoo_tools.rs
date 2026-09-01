//! The odoo tool contract: what the server advertises, and what it says when
//! asked for something it does not have.

use systemprompt_mcp_odoo::server::tool::unknown_tool;
use systemprompt_mcp_odoo::tools::{ALL_TOOLS, SERVER_NAME, TOOL_LEAD_DELETE, list_tools};

#[test]
fn every_declared_tool_is_advertised() {
    let advertised: Vec<String> = list_tools().iter().map(|t| t.name.to_string()).collect();

    for name in ALL_TOOLS {
        assert!(
            advertised.iter().any(|a| a == name),
            "{name} is dispatchable but not listed, so no client would ever call it"
        );
    }
    assert_eq!(advertised.len(), ALL_TOOLS.len());
}

#[test]
fn every_tool_carries_an_input_and_output_schema() {
    for tool in list_tools() {
        assert!(
            !tool.input_schema.is_empty(),
            "{} has an empty input schema",
            tool.name
        );
        let output = tool
            .output_schema
            .as_ref()
            .unwrap_or_else(|| panic!("{} declares no output schema", tool.name));
        assert_eq!(
            output.get("type").and_then(|t| t.as_str()),
            Some("object"),
            "{}: MCP requires outputSchema to be an object schema; Claude \
             Desktop parks the whole server on the first tool that is not",
            tool.name
        );
        assert!(
            tool.description.is_some(),
            "{} has no description for a model to select on",
            tool.name
        );
    }
}

#[test]
fn tool_meta_names_this_server() {
    let tool = list_tools().into_iter().next().expect("tools exist");
    let meta = tool.meta.expect("ui meta is attached");

    assert!(
        serde_json::to_string(&meta.0)
            .expect("meta serializes")
            .contains(SERVER_NAME),
        "the UI groups tools by server; unattributed tools land nowhere"
    );
}

#[test]
fn the_search_tool_takes_only_optional_filters() {
    let tool = list_tools()
        .into_iter()
        .find(|t| t.name.as_ref() == "crm_lead_search")
        .expect("crm_lead_search is listed");
    let schema = serde_json::to_value(&*tool.input_schema).expect("schema serializes");

    assert!(
        schema.get("required").is_none() || schema["required"].as_array().is_none_or(Vec::is_empty),
        "\"show me the pipeline\" must not require the caller to invent a filter: {schema}"
    );
}

#[test]
fn the_unknown_tool_error_lists_what_is_available() {
    let err = unknown_tool("crm_lead_merge");
    let message = err.message.to_string();

    assert!(message.contains("crm_lead_merge"), "got: {message}");
    for name in ALL_TOOLS {
        assert!(
            message.contains(name),
            "a model that guessed wrong should be able to correct itself from the error: \
             {name} missing from {message}"
        );
    }
}

#[test]
fn the_delete_tool_is_advertised_destructive_and_not_read_only() {
    let tools = list_tools();
    let delete = tools
        .iter()
        .find(|t| t.name == TOOL_LEAD_DELETE)
        .expect("crm_lead_delete is listed");
    let annotations = delete
        .annotations
        .as_ref()
        .expect("a destructive tool carries annotations");

    assert_eq!(annotations.destructive_hint, Some(true));
    assert_ne!(annotations.read_only_hint, Some(true));
    assert_eq!(
        delete.input_schema.get("required"),
        Some(&serde_json::json!(["id"]))
    );
}

#[test]
fn the_delete_tool_name_matches_the_governance_blocklist() {
    assert!(
        TOOL_LEAD_DELETE.contains("delete"),
        "the governance tool_blocklist stage matches the `delete` substring; renaming the tool \
         would silently let it through"
    );
}
