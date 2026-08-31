//! Tools exposed by the `factsheet` MCP server.
//!
//! Three tools, and the shape of them is the point. A factsheet is data, so
//! `factsheet_get` hands you that data, you change it, and `factsheet_render`
//! turns it back into a PDF. Editing a factsheet is editing a document model —
//! there is no separate "edit the PDF" step because there is no PDF to edit
//! until you ask for one.

pub mod catalog;
pub mod inputs;

use rmcp::model::{MetaObject, Tool, ToolAnnotations};
use std::sync::Arc;
use systemprompt::mcp::{McpOutputSchema, default_tool_visibility, tool_ui_meta};
use systemprompt::models::artifacts::CliArtifact;

pub const SERVER_NAME: &str = "factsheet";

pub const TOOL_LIST: &str = "factsheet_list";
pub const TOOL_GET: &str = "factsheet_get";
pub const TOOL_RENDER: &str = "factsheet_render";

pub const ALL_TOOLS: [&str; 3] = [TOOL_LIST, TOOL_GET, TOOL_RENDER];

pub(crate) struct ToolDef<'a> {
    pub name: &'a str,
    pub title: &'a str,
    pub description: &'a str,
    // JSON: protocol boundary
    pub input_schema: serde_json::Value,
    pub read_only: bool,
}

pub(crate) fn create_tool(def: &ToolDef<'_>) -> Tool {
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
    tool.annotations = def
        .read_only
        .then(|| ToolAnnotations::new().read_only(true));
    tool.meta = Some(MetaObject(tool_ui_meta(
        SERVER_NAME,
        &default_tool_visibility(),
    )));
    tool
}

#[must_use]
pub fn list_tools() -> Vec<Tool> {
    catalog::factsheet_tools()
}
