//! Inputs for the record-anchored knowledge bank: chatter and attachments.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NoteAddInput {
    pub model: String,
    pub res_id: i64,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NoteListInput {
    pub model: String,
    pub res_id: i64,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NoteSearchInput {
    pub query: String,
    pub model: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AttachmentAddInput {
    pub model: String,
    pub res_id: i64,
    pub filename: String,
    pub content_base64: Option<String>,
    pub url: Option<String>,
    pub mimetype: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AttachmentListInput {
    pub model: Option<String>,
    pub res_id: Option<i64>,
    pub query: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct AttachmentGetInput {
    pub id: i64,
}
