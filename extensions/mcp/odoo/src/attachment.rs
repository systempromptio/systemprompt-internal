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
use crate::tools::inputs::{AttachmentAddInput, AttachmentListInput};

pub const MAX_UPLOAD_BYTES: usize = 5 * 1024 * 1024;

pub const MAX_INLINE_BYTES: i64 = 1024 * 1024;

pub const ATTACHMENT_FIELDS: [&str; 10] = [
    "id",
    "name",
    "type",
    "url",
    "mimetype",
    "file_size",
    "res_model",
    "res_id",
    "create_uid",
    "create_date",
];

pub const ATTACHMENT_LABELS: [(&str, &str); 9] = [
    ("name", "Filename"),
    ("type", "Kind"),
    ("url", "URL"),
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

/// What `attachment_add` was asked to create: stored bytes, or a pointer.
///
/// Odoo models both as `ir.attachment` rows distinguished by `type`, and the
/// distinction matters downstream — a `url` row has no `datas` to read back,
/// so `attachment_get` must not offer to return content it does not have.
#[derive(Debug, PartialEq, Eq)]
pub enum Upload {
    Binary { content_base64: String, size: usize },
    Url(String),
}

pub fn classify_upload(input: &AttachmentAddInput) -> Result<Upload, McpError> {
    let content = input
        .content_base64
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty());
    let url = input
        .url
        .as_deref()
        .map(str::trim)
        .filter(|u| !u.is_empty());

    match (content, url) {
        (Some(_), Some(_)) => Err(McpError::invalid_params(
            "Provide either content_base64 or url, not both — one stores the file in Odoo, the \
             other records a pointer to it."
                .to_owned(),
            None,
        )),
        (None, None) => Err(McpError::invalid_params(
            "Provide content_base64 to store a file, or url to record a link to one held \
             elsewhere."
                .to_owned(),
            None,
        )),
        (Some(content), None) => Ok(Upload::Binary {
            size: check_upload(content)?,
            content_base64: content.to_owned(),
        }),
        (None, Some(url)) => {
            // Why: Odoo will accept any string here, so a typo becomes a dead
            // link discovered months later by whoever needed the recording.
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                return Err(McpError::invalid_params(
                    format!("url must be an http or https address; got \"{url}\"."),
                    None,
                ));
            }
            Ok(Upload::Url(url.to_owned()))
        },
    }
}

#[doc(hidden)]
#[must_use]
pub fn create_values(
    input: &AttachmentAddInput,
    filename: &str,
    upload: &Upload,
) -> serde_json::Value {
    let mut values = serde_json::Map::new();
    values.insert("name".to_owned(), serde_json::json!(filename));
    values.insert("res_model".to_owned(), serde_json::json!(input.model));
    values.insert("res_id".to_owned(), serde_json::json!(input.res_id));

    match upload {
        Upload::Binary { content_base64, .. } => {
            values.insert("type".to_owned(), serde_json::json!("binary"));
            values.insert("datas".to_owned(), serde_json::json!(content_base64));
            if let Some(mimetype) = input
                .mimetype
                .as_deref()
                .map(str::trim)
                .filter(|m| !m.is_empty())
            {
                values.insert("mimetype".to_owned(), serde_json::json!(mimetype));
            }
        },
        Upload::Url(url) => {
            // Why: no mimetype on a link. Odoo does not serve the bytes, so a
            // declared type here would describe something this row does not
            // hold.
            values.insert("type".to_owned(), serde_json::json!("url"));
            values.insert("url".to_owned(), serde_json::json!(url));
        },
    }
    serde_json::Value::Object(values)
}

#[must_use]
pub fn is_url_attachment(record: &serde_json::Value) -> bool {
    record.get("type").and_then(serde_json::Value::as_str) == Some("url")
}

#[doc(hidden)]
pub fn check_upload(content_base64: &str) -> Result<usize, McpError> {
    let decoded = STANDARD.decode(content_base64.trim()).map_err(|e| {
        McpError::invalid_params(format!("content_base64 is not valid base64: {e}"), None)
    })?;

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

#[doc(hidden)]
#[must_use]
pub fn attachment_domain(input: &AttachmentListInput) -> serde_json::Value {
    let mut domain: Vec<serde_json::Value> = Vec::new();
    if let Some(model) = input
        .model
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
    {
        domain.push(serde_json::json!(["res_model", "=", model]));
    }
    if let Some(res_id) = input.res_id {
        domain.push(serde_json::json!(["res_id", "=", res_id]));
    }
    if let Some(query) = input
        .query
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
    {
        domain.push(serde_json::json!(["name", "ilike", format!("%{query}%")]));
    }
    serde_json::Value::Array(domain)
}

#[doc(hidden)]
#[must_use]
pub fn attachment_row(record: &serde_json::Value) -> String {
    let id = record
        .get("id")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or_default();
    // Why: a link and a stored file render differently on purpose. Showing
    // "0 bytes" for a pointer would read as an upload that failed.
    let what = if is_url_attachment(record) {
        format!("link → {}", field_or_dash(record, "url"))
    } else {
        format!(
            "{} · {} bytes",
            field_or_dash(record, "mimetype"),
            field_or_dash(record, "file_size")
        )
    };
    format!(
        "- **[{id}] {}** — {what} · on {} {}",
        field_or_dash(record, "name"),
        field_or_dash(record, "res_model"),
        field_or_dash(record, "res_id"),
    )
}

#[doc(hidden)]
#[must_use]
pub fn too_large_notice(file_size: i64) -> String {
    format!(
        "\n\nContent withheld: this file is {file_size} bytes, over the {MAX_INLINE_BYTES}-byte \
         inline limit. Its metadata is above; download the file from the Odoo web UI."
    )
}
