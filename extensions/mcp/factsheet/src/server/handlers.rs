//! The three tool handlers.

use rmcp::ErrorData as McpError;
use std::sync::Arc;
use systemprompt::database::DbPool;
use systemprompt::files::FilesConfig;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::{CliArtifact, ImageArtifact, TextArtifact};
use systemprompt::models::execution::context::RequestContext;
use systemprompt_factsheet::{FactsheetDoc, FactsheetEngine};

use crate::error::ServerError;
use crate::store;
use crate::tools::inputs::{GetInput, ListInput, RenderInput};
use crate::tools::{TOOL_GET, TOOL_LIST, TOOL_RENDER};

/// Shared per-call state.
#[derive(Debug, Clone)]
pub struct Call {
    pub engine: Arc<FactsheetEngine>,
    pub db_pool: DbPool,
    pub files_config: Arc<FilesConfig>,
    /// Scratch directory for renders before they are stored.
    pub work_dir: Arc<std::path::PathBuf>,
}

#[derive(Debug)]
pub struct ListHandler {
    pub call: Call,
}

impl McpToolHandler for ListHandler {
    type Input = ListInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_LIST
    }

    fn description(&self) -> &'static str {
        "List the factsheets this instance ships."
    }

    fn handle(
        &self,
        _input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            let ids = call.engine.list_sheets().map_err(ServerError::Engine)?;
            let summary = if ids.is_empty() {
                "No factsheets are installed.".to_owned()
            } else {
                format!("{} factsheet(s): {}", ids.len(), ids.join(", "))
            };
            let body = ids
                .iter()
                .map(|id| format!("- `{id}`"))
                .collect::<Vec<_>>()
                .join("\n");
            let artifact = TextArtifact::new(body).with_title("Factsheets");
            Ok((CliArtifact::text(artifact), summary))
        }
    }
}

#[derive(Debug)]
pub struct GetHandler {
    pub call: Call,
}

impl McpToolHandler for GetHandler {
    type Input = GetInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_GET
    }

    fn description(&self) -> &'static str {
        "Return a factsheet's editable document model."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            let doc = call.engine.load_sheet(&input.id).map_err(ServerError::Engine)?;
            let yaml = serde_yaml::to_string(&doc)
                .map_err(|e| ServerError::Internal(format!("serialising sheet: {e}")))?;
            let blocks: usize = doc.pages.iter().map(|page| page.blocks.len()).sum();
            let summary = format!(
                "Factsheet '{}': {} page(s), {} block(s), budget {} page(s).",
                doc.id,
                doc.pages.len(),
                blocks,
                doc.max_pages
            );
            let body = format!("```yaml\n{yaml}```");
            let artifact = TextArtifact::new(body).with_title(doc.title);
            Ok((CliArtifact::text(artifact), summary))
        }
    }
}

#[derive(Debug)]
pub struct RenderHandler {
    pub call: Call,
}

impl McpToolHandler for RenderHandler {
    type Input = RenderInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_RENDER
    }

    fn description(&self) -> &'static str {
        "Render a factsheet to a stored PDF with page previews."
    }

    fn handle(
        &self,
        input: Self::Input,
        ctx: &RequestContext,
        exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        let ctx = ctx.clone();
        let exec_id = exec_id.clone();
        async move {
            let doc = resolve_doc(&call.engine, input)?;
            // Why: one directory per execution. Two renders of the same sheet
            // running concurrently would otherwise write the same filenames.
            let work_dir = call.work_dir.join(exec_id.as_str());
            let rendered = call
                .engine
                .render_pdf(&doc, &work_dir)
                .await
                .map_err(ServerError::Engine)?;

            let stored = store::store(&call.db_pool, &call.files_config, &rendered, &ctx).await?;

            // Best effort: the render succeeded, so failing to tidy the scratch
            // directory must not fail the call.
            if let Err(error) = tokio::fs::remove_dir_all(&work_dir).await {
                tracing::warn!(
                    dir = %work_dir.display(),
                    %error,
                    "could not remove factsheet scratch directory"
                );
            }

            let pages = stored
                .pages
                .iter()
                .enumerate()
                .map(|(index, page)| format!("  - page {}: {}", index + 1, page.public_url))
                .collect::<Vec<_>>()
                .join("\n");

            let summary = format!(
                "Rendered factsheet '{}' — {} page(s), {} KB.\nPDF: {}\n{}",
                doc.id,
                stored.page_count,
                stored.pdf.size_bytes / 1024,
                stored.pdf.public_url,
                pages
            );

            let caption = format!(
                "{} · page 1 of {} · PDF: {}",
                doc.title, stored.page_count, stored.pdf.public_url
            );
            let preview = stored
                .pages
                .first()
                .map_or_else(|| stored.pdf.public_url.clone(), |page| page.public_url.clone());

            let artifact = ImageArtifact::new(preview)
                .with_alt(format!("Factsheet {}, page 1", doc.id))
                .with_caption(caption)
                .with_request(&ctx);

            Ok((CliArtifact::image(artifact), summary))
        }
    }
}

/// An inline document wins over a sheet id; one of the two must be present.
fn resolve_doc(engine: &FactsheetEngine, input: RenderInput) -> Result<FactsheetDoc, ServerError> {
    if let Some(doc) = input.doc {
        return Ok(doc);
    }
    let Some(id) = input.sheet_id else {
        return Err(ServerError::Internal(
            "Pass either `sheet_id` to render a sheet as it ships, or `doc` to render one you \
             have edited."
                .to_owned(),
        ));
    };
    engine.load_sheet(&id).map_err(ServerError::Engine)
}
