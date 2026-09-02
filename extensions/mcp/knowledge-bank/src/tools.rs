//! Tool definitions exposed by the `knowledge-bank` MCP server.
//!
//! Six tools. `search_project_context` and `list_documents` read the bank;
//! `upload_document` adds to it; `proposal_list`, `proposal_get` and
//! `proposal_decide` drive the brain@ → Odoo projection queue. Uploads and
//! everything under `proposal_*` require the admin role (enforced in
//! `server::tool`) — proposals carry inbound business email verbatim.
//!
//! The input field names mirror the `knowledge_documents` columns — `source`,
//! `project` — so a caller reading a search result already knows what to pass
//! back to narrow the next one. The `proposal_*` tools return typed
//! `structuredContent` rather than prose: dashboards consume them, and a
//! dashboard must never have to regex a sentence back apart.

use rmcp::model::{MetaObject, Tool, ToolAnnotations};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt::mcp::{McpOutputSchema, default_tool_visibility, tool_ui_meta};
use systemprompt::models::artifacts::CliArtifact;

use crate::server::tool::proposal_outputs::{
    ProposalDecideOutput, ProposalGetOutput, ProposalListOutput,
};

pub const SERVER_NAME: &str = "knowledge-bank";
pub const TOOL_SEARCH: &str = "search_project_context";
pub const TOOL_LIST: &str = "list_documents";
pub const TOOL_UPLOAD: &str = "upload_document";
pub const TOOL_PROPOSAL_LIST: &str = "proposal_list";
pub const TOOL_PROPOSAL_GET: &str = "proposal_get";
pub const TOOL_PROPOSAL_DECIDE: &str = "proposal_decide";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchInput {
    pub query: String,
    pub project: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListInput {
    pub project: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UploadInput {
    pub title: String,
    pub source: String,
    pub project: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProposalListInput {
    pub status: Option<String>,
    pub query: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProposalGetInput {
    pub document_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DecisionInput {
    Approve,
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProposalDecideInput {
    pub document_id: String,
    pub decision: DecisionInput,
    #[serde(default)]
    pub exclude_actions: Vec<usize>,
    pub note: Option<String>,
}

struct ToolDef<'a> {
    name: &'a str,
    title: &'a str,
    description: &'a str,
    // JSON: protocol boundary
    input_schema: serde_json::Value,
    // JSON: protocol boundary
    output_schema: serde_json::Value,
    read_only: bool,
}

fn create_tool(def: &ToolDef<'_>) -> Tool {
    let input_obj = def
        .input_schema
        .as_object()
        .cloned()
        .unwrap_or_else(serde_json::Map::new);
    let output_obj = def
        .output_schema
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
            output_schema: <CliArtifact as McpOutputSchema>::validated_schema(),
            read_only: true,
        }),
        create_tool(&ToolDef {
            name: TOOL_LIST,
            title: "List Knowledge Bank Documents",
            description: "List knowledge bank documents newest first, optionally filtered by \
                          project and source. Returns titles and sizes, not content — search \
                          for the content.",
            input_schema: schemars::schema_for!(ListInput).to_value(),
            output_schema: <CliArtifact as McpOutputSchema>::validated_schema(),
            read_only: true,
        }),
        create_tool(&ToolDef {
            name: TOOL_UPLOAD,
            title: "Upload Document",
            description: "Add a document (meeting transcript, note, or page) to the company \
                          knowledge bank, where it becomes searchable immediately. Admin role \
                          required.",
            input_schema: schemars::schema_for!(UploadInput).to_value(),
            output_schema: <CliArtifact as McpOutputSchema>::validated_schema(),
            read_only: false,
        }),
        create_tool(&ToolDef {
            name: TOOL_PROPOSAL_LIST,
            title: "List Ingestion Proposals",
            description: "The brain@ email feed: every captured email with its pipeline state \
                          (raw, categorized, skipped, proposed, approved, applied, failed, \
                          denied, expired), category, summary and the proposed Odoo actions. \
                          Filter by status or search by sender/subject. Also reports whether \
                          the caller can apply proposals (a linked Odoo account). Admin only.",
            input_schema: schemars::schema_for!(ProposalListInput).to_value(),
            output_schema: <ProposalListOutput as McpOutputSchema>::validated_schema(),
            read_only: true,
        }),
        create_tool(&ToolDef {
            name: TOOL_PROPOSAL_GET,
            title: "Get Ingestion Proposal",
            description: "One captured email in full: the proposal, what was applied, and the \
                          body as it would be logged in Odoo chatter. Admin only.",
            input_schema: schemars::schema_for!(ProposalGetInput).to_value(),
            output_schema: <ProposalGetOutput as McpOutputSchema>::validated_schema(),
            read_only: true,
        }),
        create_tool(&ToolDef {
            name: TOOL_PROPOSAL_DECIDE,
            title: "Decide Ingestion Proposal",
            description: "Approve or reject one proposed Odoo projection. Approving applies the \
                          actions immediately as the caller's own Odoo account and returns what \
                          landed; `exclude_actions` drops individual actions by index. Deciding \
                          resolves the same approval_requests row that /admin/governance/approvals \
                          shows. Admin only.",
            input_schema: schemars::schema_for!(ProposalDecideInput).to_value(),
            output_schema: <ProposalDecideOutput as McpOutputSchema>::validated_schema(),
            read_only: false,
        }),
    ]
}
