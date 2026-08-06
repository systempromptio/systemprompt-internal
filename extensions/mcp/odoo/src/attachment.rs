//! Attachment rules: size limits, the search domain, and rendering.
//!
//! Separated from the handlers in [`crate::server::attachments`] because these
//! are the parts worth asserting directly — a size gate that is off by a factor
//! of 1024, or a domain that silently drops a filter, fails in ways an
//! integration test against a live Odoo would not reliably catch.
//!
//! Two size limits, for different reasons. [`MAX_UPLOAD_BYTES`] guards what the
//! caller may push into Odoo. [`MAX_INLINE_BYTES`] is much smaller and guards
//! the *caller*: attachment bodies come back base64-encoded through the model's
//! context window, and a multi-megabyte file would exhaust it to no purpose.
//! Both are enforced here rather than left to Odoo, which would accept either
//! and only fail later.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use rmcp::ErrorData as McpError;

use crate::format::field_or_dash;
use crate::tools::inputs::AttachmentListInput;

/// Largest file `attachment_add` will upload, decoded: 5 MB.
pub const MAX_UPLOAD_BYTES: usize = 5 * 1024 * 1024;

/// Largest file `attachment_get` will return inline, in bytes: 1 MB.
pub const MAX_INLINE_BYTES: i64 = 1024 * 1024;

pub const ATTACHMENT_FIELDS: [&str; 8] = [
    "id",
    "name",
    "mimetype",
    "file_size",
    "res_model",
    "res_id",
    "create_uid",
    "create_date",
];

pub const ATTACHMENT_LABELS: [(&str, &str); 7] = [
    ("name", "Filename"),
    ("mimetype", "MIME type"),
    ("file_size", "Size (bytes)"),
    ("res_model", "Attached to (model)"),
    ("res_id", "Attached to (id)"),
    ("create_uid", "Uploaded by"),
    ("create_date", "Uploaded"),
];

pub fn attachment_fields() -> Vec<String> {
    ATTACHMENT_FIELDS.iter().map(|f| (*f).to_owned()).collect()
}

/// Validate an upload and report its decoded size.
///
/// Returns the decoded byte length on success. The decoded bytes themselves are
/// dropped: Odoo wants the base64 form, so decoding here is a validation step,
/// not a conversion — it proves the payload is really base64 and measures what
/// it will cost before anything crosses the wire.
///
/// # Errors
/// A payload that is not valid base64, is empty, or exceeds
/// [`MAX_UPLOAD_BYTES`] once decoded.
#[doc(hidden)]
pub fn check_upload(content_base64: &str) -> Result<usize, McpError> {
    let decoded = STANDARD
        .decode(content_base64.trim())
        .map_err(|e| McpError::invalid_params(format!("content_base64 is not valid base64: {e}"), None))?;

    if decoded.is_empty() {
        return Err(McpError::invalid_params(
            "content_base64 decodes to an empty file.".to_owned(),
            None,
        ));
    }
    if decoded.len() > MAX_UPLOAD_BYTES {
        return Err(McpError::invalid_params(
            format!(
                "File is {} bytes decoded, over the {MAX_UPLOAD_BYTES}-byte upload limit. Upload \
                 it through the Odoo web UI instead.",
                decoded.len()
            ),
            None,
        ));
    }
    Ok(decoded.len())
}

/// The attachment search domain: model, record, and filename filters.
///
/// `res_id` without `model` is accepted but nearly meaningless, since Odoo ids
/// are only unique within a model — the tool description says so rather than
/// rejecting it, because the caller may genuinely want every model's id 42.
///
/// Exposed (behind `#[doc(hidden)]`) for the external test workspace; not part
/// of the public API.
#[doc(hidden)]
#[must_use]
pub fn attachment_domain(input: &AttachmentListInput) -> serde_json::Value {
    let mut domain: Vec<serde_json::Value> = Vec::new();
    if let Some(model) = input.model.as_deref().map(str::trim).filter(|m| !m.is_empty()) {
        domain.push(serde_json::json!(["res_model", "=", model]));
    }
    if let Some(res_id) = input.res_id {
        domain.push(serde_json::json!(["res_id", "=", res_id]));
    }
    if let Some(query) = input.query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        domain.push(serde_json::json!(["name", "ilike", format!("%{query}%")]));
    }
    serde_json::Value::Array(domain)
}

/// One attachment as a markdown list row.
#[doc(hidden)]
#[must_use]
pub fn attachment_row(record: &serde_json::Value) -> String {
    let id = record
        .get("id")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or_default();
    format!(
        "- **[{id}] {}** — {} · {} bytes · on {} {}",
        field_or_dash(record, "name"),
        field_or_dash(record, "mimetype"),
        field_or_dash(record, "file_size"),
        field_or_dash(record, "res_model"),
        field_or_dash(record, "res_id"),
    )
}

/// The message returned in place of a body that is too large to inline.
#[doc(hidden)]
#[must_use]
pub fn too_large_notice(file_size: i64) -> String {
    format!(
        "\n\nContent withheld: this file is {file_size} bytes, over the {MAX_INLINE_BYTES}-byte \
         inline limit. Its metadata is above; download the file from the Odoo web UI."
    )
}
