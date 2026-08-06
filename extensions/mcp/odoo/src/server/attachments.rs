//! The attachment tools: `attachment_add`, `attachment_list`,
//! `attachment_get`.
//!
//! `ir.attachment` is the other half of the record-anchored knowledge bank.
//! Like chatter, an attachment carries a `(res_model, res_id)` pair, so a file
//! is filed against the lead or partner it concerns rather than into a folder
//! nobody can find.
//!
//! The size limits and the search domain live in [`crate::attachment`]; this
//! module is the three handlers and the two reads behind `attachment_get`.

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext;

use super::call::OdooCall;
use crate::attachment::{
    ATTACHMENT_FIELDS, ATTACHMENT_LABELS, MAX_INLINE_BYTES, attachment_domain, attachment_fields,
    attachment_row, check_upload, too_large_notice,
};
use crate::client::SearchOptions;
use crate::format::{detail_lines, empty_result, text_artifact};
use crate::tools::inputs::{
    AttachmentAddInput, AttachmentGetInput, AttachmentListInput, resolve_limit,
};
use crate::tools::{TOOL_ATTACHMENT_ADD, TOOL_ATTACHMENT_GET, TOOL_ATTACHMENT_LIST};

#[derive(Debug)]
pub struct AttachmentAddHandler {
    pub call: OdooCall,
}

impl McpToolHandler for AttachmentAddHandler {
    type Input = AttachmentAddInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_ATTACHMENT_ADD
    }

    fn description(&self) -> &'static str {
        "Attach a file to an Odoo record."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            let filename = input.filename.trim().to_owned();
            if filename.is_empty() {
                return Err(McpError::invalid_params(
                    "A filename is required.".to_owned(),
                    None,
                ));
            }
            let size = check_upload(&input.content_base64)?;

            let mut values = serde_json::Map::new();
            values.insert("name".to_owned(), serde_json::json!(filename));
            values.insert("res_model".to_owned(), serde_json::json!(input.model));
            values.insert("res_id".to_owned(), serde_json::json!(input.res_id));
            values.insert(
                "datas".to_owned(),
                serde_json::json!(input.content_base64.trim()),
            );
            if let Some(mimetype) = input.mimetype.map(|m| m.trim().to_owned()).filter(|m| !m.is_empty())
            {
                values.insert("mimetype".to_owned(), serde_json::json!(mimetype));
            }

            let id = call
                .client
                .create(
                    &call.creds,
                    "ir.attachment",
                    serde_json::Value::Object(values),
                )
                .await?;

            let summary = format!(
                "Attached {filename} ({size} bytes) to {} {} as attachment {id}",
                input.model, input.res_id
            );
            Ok((text_artifact("Attachment Created", &summary), summary))
        }
    }
}

#[derive(Debug)]
pub struct AttachmentListHandler {
    pub call: OdooCall,
}

impl McpToolHandler for AttachmentListHandler {
    type Input = AttachmentListInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_ATTACHMENT_LIST
    }

    fn description(&self) -> &'static str {
        "List Odoo attachments, optionally scoped to a record or filename."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            let options = SearchOptions {
                fields: attachment_fields(),
                limit: resolve_limit(input.limit),
                order: Some("create_date desc".to_owned()),
            };
            let records = call
                .client
                .search_read(
                    &call.creds,
                    "ir.attachment",
                    attachment_domain(&input),
                    &options,
                )
                .await?;

            let summary = format!("{} attachment(s) matched", records.len());
            let body = if records.is_empty() {
                empty_result("attachments")
            } else {
                records
                    .iter()
                    .map(attachment_row)
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            Ok((text_artifact("Odoo Attachments", &body), summary))
        }
    }
}

#[derive(Debug)]
pub struct AttachmentGetHandler {
    pub call: OdooCall,
}

impl McpToolHandler for AttachmentGetHandler {
    type Input = AttachmentGetInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_ATTACHMENT_GET
    }

    fn description(&self) -> &'static str {
        "Read one Odoo attachment, with its content when the file is small."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            // Why: metadata first, content second. Reading `datas` up front
            // would pull a base64 blob of unknown size across the wire only to
            // discard it, which is the exact cost the inline limit exists to
            // avoid.
            let metadata = read_metadata(&call, input.id).await?;
            let file_size = metadata
                .get("file_size")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or_default();

            let mut body = detail_lines(&metadata, &ATTACHMENT_LABELS);
            let summary = if file_size > MAX_INLINE_BYTES {
                body.push_str(&too_large_notice(file_size));
                format!("Attachment {} metadata read; content withheld", input.id)
            } else {
                let content = read_content(&call, input.id).await?;
                body.push_str(&format!("\n\n```base64\n{content}\n```"));
                format!("Attachment {} read ({file_size} bytes)", input.id)
            };
            Ok((text_artifact("Odoo Attachment", &body), summary))
        }
    }
}

async fn read_metadata(call: &OdooCall, id: i64) -> Result<serde_json::Value, McpError> {
    let fields: Vec<&str> = ATTACHMENT_FIELDS.to_vec();
    let mut records = call
        .client
        .read(&call.creds, "ir.attachment", &[id], &fields)
        .await?;
    records.pop().ok_or_else(|| {
        McpError::invalid_params(
            format!("No attachment with id {id} is visible to your Odoo account."),
            None,
        )
    })
}

async fn read_content(call: &OdooCall, id: i64) -> Result<String, McpError> {
    let records = call
        .client
        .read(&call.creds, "ir.attachment", &[id], &["datas"])
        .await?;
    Ok(records
        .first()
        .and_then(|r| r.get("datas"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned())
}
