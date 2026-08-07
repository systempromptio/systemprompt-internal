//! Tool definitions exposed by the `knowledge-bank` MCP server.
//!
//! Three tools: `search_project_context` and `list_documents` for any
//! signed-in user, and `upload_document` restricted to admins (enforced in
//! `server::tool`).
//!
//! The input field names mirror the `knowledge_documents` columns — `source`,
//! `project` — so a caller reading a search result already knows what to pass
//! back to narrow the next one.

use rmcp::model::{MetaObject, Tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt::mcp::{McpOutputSchema, default_tool_visibility, tool_ui_meta};
use systemprompt::models::artifacts::CliArtifact;

pub const SERVER_NAME: &str = "knowledge-bank";
pub const TOOL_SEARCH: &str = "search_project_context";
pub const TOOL_LIST: &str = "list_documents";
pub const TOOL_UPLOAD: &str = "upload_document";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchInput {
    /// Free-text query. Supports quoted phrases and `-exclusions`. Leave it
    /// empty to see the most recently uploaded documents instead.
    pub query: String,
    /// Optional project tag to scope the search to one collection.
    pub project: Option<String>,
    /// Maximum number of documents to return (default 10, maximum 50).
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListInput {
    /// Optional project tag to scope the listing.
    pub project: Option<String>,
    /// Optional source filter, e.g. "meeting-transcript", "document", "email".
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UploadInput {
    /// Human-readable document title.
    pub title: String,
    /// Where the document came from, e.g. "meeting-transcript", "document",
    /// "email". Freeform — reuse an existing value so listings stay filterable.
    pub source: String,
    /// Optional project tag grouping this document with related ones.
    pub project: Option<String>,
    /// Full document text. At most 2 MB.
    pub content: String,
}

struct ToolDef<'a> {
    name: &'a str,
    title: &'a str,
    description: &'a str,
    // JSON: protocol boundary
    input_schema: serde_json::Value,
}

fn create_tool(def: &ToolDef<'_>) -> Tool {
    let input_obj = def
        .input_schema
        .as_object()
        .cloned()
        .unwrap_or_else(serde_json::Map::new);
    let output_obj = <CliArtifact as McpOutputSchema>::validated_schema()
        .as_object()
        .cloned()
        .unwrap_or_else(serde_json::Map::new);

    let mut tool = Tool::default();
    tool.name = def.name.to_owned().into();
    tool.title = Some(def.title.to_owned());
    tool.description = Some(def.description.to_owned().into());
    tool.input_schema = Arc::new(input_obj);
    tool.output_schema = Some(Arc::new(output_obj));
    tool.meta = Some(MetaObject(tool_ui_meta(
        SERVER_NAME,
        &default_tool_visibility(),
    )));
    tool
}

#[must_use]
pub fn list_tools() -> Vec<Tool> {
    vec![
        create_tool(&ToolDef {
            name: TOOL_SEARCH,
            title: "Search Project Context",
            description: "Full-text search across the company knowledge bank — meeting \
                          transcripts, documents and notes — for prior decisions and context. \
                          Returns ranked snippets, not whole documents. Use this before \
                          proposing an approach: prior decisions recorded here outrank general \
                          best practice.",
            input_schema: schemars::schema_for!(SearchInput).to_value(),
        }),
        create_tool(&ToolDef {
            name: TOOL_LIST,
            title: "List Knowledge Bank Documents",
            description: "List knowledge bank documents newest first, optionally filtered by \
                          project and source. Returns titles and sizes, not content — search \
                          for the content.",
            input_schema: schemars::schema_for!(ListInput).to_value(),
        }),
        create_tool(&ToolDef {
            name: TOOL_UPLOAD,
            title: "Upload Document",
            description: "Add a document (meeting transcript, note, or page) to the company \
                          knowledge bank, where it becomes searchable immediately. Admin role \
                          required.",
            input_schema: schemars::schema_for!(UploadInput).to_value(),
        }),
    ]
}
