//! `list_tools` is the wire contract the real RAG server must satisfy when it
//! replaces the stub: exactly three tools with stable names and titles, an
//! input schema that makes `query` mandatory on search and everything optional
//! on list, a `ToolResponse` output schema on every tool, and UI meta so the
//! client knows which server rendered the call.

use systemprompt_mcp_knowledge_bank::tools::{
    ListInput, SearchInput, TOOL_LIST, TOOL_SEARCH, TOOL_UPLOAD, UploadInput, list_tools,
};

fn required_fields(schema: &serde_json::Map<String, serde_json::Value>) -> Vec<String> {
    schema
        .get("required")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn exposes_exactly_the_three_contract_tools_in_order() {
    let tools = list_tools();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert_eq!(names, vec![TOOL_SEARCH, TOOL_LIST, TOOL_UPLOAD]);
}

#[test]
fn every_tool_carries_a_title_and_a_description() {
    for tool in list_tools() {
        let title = tool.title.as_deref().expect("title is set");
        assert!(!title.is_empty(), "{} has an empty title", tool.name);
        let description = tool.description.as_deref().expect("description is set");
        assert!(
            !description.is_empty(),
            "{} has an empty description",
            tool.name
        );
    }
}

#[test]
fn search_requires_query_and_leaves_the_other_inputs_optional() {
    let tools = list_tools();
    let search = tools
        .iter()
        .find(|t| t.name.as_ref() == TOOL_SEARCH)
        .expect("search tool listed");
    let required = required_fields(&search.input_schema);
    assert_eq!(required, vec!["query".to_owned()]);
}

#[test]
fn upload_requires_all_four_fields_and_list_requires_none() {
    let tools = list_tools();
    let upload = tools
        .iter()
        .find(|t| t.name.as_ref() == TOOL_UPLOAD)
        .expect("upload tool listed");
    let mut required = required_fields(&upload.input_schema);
    required.sort();
    assert_eq!(required, vec!["content", "doc_type", "project_id", "title"]);

    let list = tools
        .iter()
        .find(|t| t.name.as_ref() == TOOL_LIST)
        .expect("list tool listed");
    assert!(required_fields(&list.input_schema).is_empty());
}

#[test]
fn every_tool_declares_an_output_schema_and_ui_meta() {
    for tool in list_tools() {
        let output = tool
            .output_schema
            .as_ref()
            .unwrap_or_else(|| panic!("{} declares an output schema", tool.name));
        assert!(
            !output.is_empty(),
            "{} output schema must not be empty",
            tool.name
        );
        assert!(
            tool.meta.is_some(),
            "{} must carry UI meta so the client can attribute the call",
            tool.name
        );
    }
}

#[test]
fn search_input_omits_optional_fields_on_the_wire() {
    let input: SearchInput =
        serde_json::from_value(serde_json::json!({ "query": "checkout" })).expect("minimal search");
    assert_eq!(input.query, "checkout");
    assert!(input.project_id.is_none());
    assert!(input.limit.is_none());
}

#[test]
fn list_input_accepts_an_empty_object() {
    let input: ListInput = serde_json::from_value(serde_json::json!({})).expect("empty list input");
    assert!(input.project_id.is_none());
    assert!(input.doc_type.is_none());
}

#[test]
fn upload_input_round_trips_and_rejects_a_missing_field() {
    let payload = serde_json::json!({
        "doc_type": "jira",
        "project_id": "acme-storefront",
        "title": "ACME-9: Spike",
        "content": "Spike outcome recorded.",
    });
    let input: UploadInput = serde_json::from_value(payload.clone()).expect("full upload input");
    assert_eq!(input.doc_type, "jira");
    assert_eq!(serde_json::to_value(&input).expect("serializes"), payload);

    let mut missing = payload;
    missing
        .as_object_mut()
        .expect("object")
        .remove("content")
        .expect("content was present");
    assert!(serde_json::from_value::<UploadInput>(missing).is_err());
}
