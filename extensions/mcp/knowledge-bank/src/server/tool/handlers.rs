//! The three tool handlers.
//!
//! Each is a thin adapter: normalise the caller's arguments, apply the gates
//! that belong to the tool rather than to the transport, hand the work to the
//! store, and render what comes back. The policy they enforce lives in
//! `store::query`; the rendering lives in `super::render`.

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::{CliArtifact, TextArtifact};
use systemprompt::models::execution::context::RequestContext as SysRequestContext;

use crate::store::{
    KnowledgeStore, NewDocument, check_content_size, clamp_search_limit, normalize_optional,
    require_non_empty,
};
use crate::tools::{ListInput, SearchInput, TOOL_LIST, TOOL_SEARCH, TOOL_UPLOAD, UploadInput};

use super::read_scope;
use super::render::{listing_summary, project_label, search_summary};

fn text_artifact(title: &str, body: &str) -> CliArtifact {
    CliArtifact::text(TextArtifact::new(body).with_title(title))
}

pub(super) struct SearchHandler {
    pub(super) store: KnowledgeStore,
}

impl McpToolHandler for SearchHandler {
    type Input = SearchInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_SEARCH
    }

    fn description(&self) -> &'static str {
        "Search the company knowledge bank."
    }

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &SysRequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(Self::Output, String), McpError> {
        let limit = clamp_search_limit(input.limit);
        let project = normalize_optional(input.project);
        let hits = self
            .store
            .search(&input.query, project.as_deref(), limit, read_scope(ctx))
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let summary = if input.query.trim().is_empty() {
            format!("{} most recent document(s)", hits.len())
        } else {
            format!("{} document(s) matched \"{}\"", hits.len(), input.query)
        };
        let body = search_summary(&hits);
        Ok((text_artifact("Project Context Search", &body), summary))
    }
}

pub(super) struct ListHandler {
    pub(super) store: KnowledgeStore,
}

impl McpToolHandler for ListHandler {
    type Input = ListInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_LIST
    }

    fn description(&self) -> &'static str {
        "List knowledge bank documents."
    }

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &SysRequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(Self::Output, String), McpError> {
        let project = normalize_optional(input.project);
        let source = normalize_optional(input.source);
        let documents = self
            .store
            .list_documents(project.as_deref(), source.as_deref(), read_scope(ctx))
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let summary = format!("{} document(s) in the knowledge bank", documents.len());
        let body = listing_summary(&documents);
        Ok((text_artifact("Knowledge Bank Documents", &body), summary))
    }
}

pub(super) struct UploadHandler {
    pub(super) store: KnowledgeStore,
}

impl McpToolHandler for UploadHandler {
    type Input = UploadInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_UPLOAD
    }

    fn description(&self) -> &'static str {
        "Upload a document to the knowledge bank (admin only)."
    }

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &SysRequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(Self::Output, String), McpError> {
        let title = require_non_empty("title", &input.title)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        let source = require_non_empty("source", &input.source)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        require_non_empty("content", &input.content)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        check_content_size(&input.content)
            .map_err(|e| McpError::invalid_params(e.to_string(), None))?;

        let project = normalize_optional(input.project);
        // Why: attribution comes from the authenticated caller, never from
        // the payload — a client that could name its own uploader could forge
        // provenance on every document in the bank.
        let uploaded_by = ctx.user_id().to_string();

        let uploaded = self
            .store
            .insert(NewDocument {
                title: &title,
                source: &source,
                project: project.as_deref(),
                content: &input.content,
                uploaded_by: &uploaded_by,
            })
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let summary = format!("Document {} uploaded to the knowledge bank", uploaded.id);
        // Why: an id alone is not actionable when the only retrieval tool is
        // a text search, so the receipt spells out how to find the document
        // again.
        let body = format!(
            "{summary}\n\ntitle: {title}\nsource: {source}\nproject: {}\nuploaded by: \
             {uploaded_by}\ncreated: {}\n\nFind it again with search_project_context (query \
             \"{title}\"{}), or list_documents with source \"{source}\".",
            project_label(project.as_deref()),
            uploaded.created_at.to_rfc3339(),
            project
                .as_deref()
                .map_or_else(String::new, |p| format!(", project \"{p}\""))
        );
        Ok((text_artifact("Document Uploaded", &body), summary))
    }
}
