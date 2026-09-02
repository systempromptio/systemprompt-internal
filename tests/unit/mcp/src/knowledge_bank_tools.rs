//! `list_tools` is the knowledge bank's wire contract: exactly six tools
//! with stable names and titles, an input schema that makes `query` mandatory
//! on search and everything optional on list, a typed output schema on every
//! tool (`CliArtifact` for the bank, JSON payloads for the proposal queue),
//! and UI meta so the client knows which server rendered the call. The input
//! field names mirror the `knowledge_documents` columns, so a caller can feed a
//! search result straight back into the next call.

use systemprompt_mcp_knowledge_bank::tools::{
    DecisionInput, ListInput, ProposalDecideInput, SearchInput, TOOL_LIST, TOOL_PROPOSAL_DECIDE,
    TOOL_PROPOSAL_GET, TOOL_PROPOSAL_LIST, TOOL_SEARCH, TOOL_UPLOAD, UploadInput, list_tools,
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
fn exposes_exactly_the_six_contract_tools_in_order() {
    let tools = list_tools();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert_eq!(
        names,
        vec![
            TOOL_SEARCH,
            TOOL_LIST,
            TOOL_UPLOAD,
            TOOL_PROPOSAL_LIST,
            TOOL_PROPOSAL_GET,
            TOOL_PROPOSAL_DECIDE
        ]
    );
}

#[test]
fn only_the_writes_drop_the_read_only_annotation() {
    for tool in list_tools() {
        let read_only = tool.annotations.as_ref().and_then(|a| a.read_only_hint) == Some(true);
        let is_write =
            tool.name.as_ref() == TOOL_UPLOAD || tool.name.as_ref() == TOOL_PROPOSAL_DECIDE;
        assert_eq!(read_only, !is_write, "{} read-only annotation", tool.name);
    }
}

#[test]
fn proposal_outputs_advertise_their_own_schema_not_the_cli_artifact() {
    let tools = list_tools();
    let list = tools
        .iter()
        .find(|t| t.name.as_ref() == TOOL_PROPOSAL_LIST)
        .expect("proposal_list listed");
    let output = list.output_schema.as_ref().expect("output schema");
    assert_eq!(
        output.get("x-artifact-type").and_then(|v| v.as_str()),
        Some("knowledge_proposal_list")
    );
    assert!(
        output["properties"].get("rows").is_some(),
        "rows is a top-level property"
    );
    assert!(
        output["properties"].get("viewer").is_some(),
        "viewer capability is reported"
    );
}

#[test]
fn decide_input_defaults_exclusions_and_pins_the_decision_enum() {
    let input: ProposalDecideInput = serde_json::from_value(serde_json::json!({
        "document_id": "00000000-0000-4000-8000-000000000001",
        "decision": "approve"
    }))
    .expect("minimal decide input");
    assert_eq!(input.decision, DecisionInput::Approve);
    assert!(input.exclude_actions.is_empty());
    assert!(input.note.is_none());
    assert!(
        serde_json::from_value::<ProposalDecideInput>(serde_json::json!({
            "document_id": "x", "decision": "maybe"
        }))
        .is_err(),
        "the decision is a closed enum"
    );
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
fn upload_requires_everything_but_project_and_list_requires_none() {
    let tools = list_tools();
    let upload = tools
        .iter()
        .find(|t| t.name.as_ref() == TOOL_UPLOAD)
        .expect("upload tool listed");
    let mut required = required_fields(&upload.input_schema);
    required.sort();
    // `project` is a collection tag, not a foreign key: a document that
    // belongs to no project is still worth banking.
    assert_eq!(required, vec!["content", "source", "title"]);

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
        assert_eq!(
            output.get("type").and_then(|t| t.as_str()),
            Some("object"),
            "{}: MCP requires outputSchema to be an object schema; Claude \
             Desktop parks the whole server on the first tool that is not",
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
    assert!(input.project.is_none());
    assert!(input.limit.is_none());
}

#[test]
fn list_input_accepts_an_empty_object() {
    let input: ListInput = serde_json::from_value(serde_json::json!({})).expect("empty list input");
    assert!(input.project.is_none());
    assert!(input.source.is_none());
}

#[test]
fn upload_input_round_trips_and_rejects_a_missing_field() {
    let payload = serde_json::json!({
        "title": "ACME-9: Spike",
        "source": "meeting-transcript",
        "project": "acme-storefront",
        "content": "Spike outcome recorded.",
    });
    let input: UploadInput = serde_json::from_value(payload.clone()).expect("full upload input");
    assert_eq!(input.source, "meeting-transcript");
    assert_eq!(serde_json::to_value(&input).expect("serializes"), payload);

    let mut missing = payload;
    missing
        .as_object_mut()
        .expect("object")
        .remove("content")
        .expect("content was present");
    assert!(serde_json::from_value::<UploadInput>(missing).is_err());
}
