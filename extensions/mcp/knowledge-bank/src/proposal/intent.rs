//! The structured shape the categorization prompt returns, shared by the job
//! that asks for it and the planner that consumes it.
//!
//! `crm_intent` is deliberately flat — no unions — so it round-trips through
//! every provider's constrained-output mode. Which Odoo records exist is not
//! the model's to know; it states what the email *is*, and the planner decides
//! what that becomes.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CATEGORIES: &[&str] = &[
    "sales",
    "client",
    "product",
    "operations",
    "finance",
    "legal",
    "technical",
    "recruiting",
    "newsletter",
    "notification",
    "spam",
    "other",
];

pub const NOISE_CATEGORIES: &[&str] = &["spam", "newsletter", "notification"];

/// `knowledge_documents.structured`, as written by categorization.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StructuredSummary {
    pub summary: String,
    #[serde(default)]
    pub entities: Vec<Entity>,
    #[serde(default)]
    pub action_items: Vec<String>,
    #[serde(default)]
    pub crm_intent: Option<CrmIntent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Entity {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CrmIntent {
    pub disposition: Disposition,
    pub lead_title: Option<String>,
    pub contact_name: Option<String>,
    pub company_name: Option<String>,
    pub note_summary: String,
    #[serde(default)]
    pub tasks: Vec<IntentTask>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    Opportunity,
    ExistingRelationship,
    Internal,
    // Why: a value outside the enum is treated as noise, never as an
    // opportunity — the failure direction that creates no CRM record.
    #[serde(other)]
    Noise,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IntentTask {
    pub title: String,
    pub due_date: Option<String>,
    pub detail: String,
}

// Why: the wire schema is hand-built rather than derived so it stays inside
// the subset every provider's strict mode accepts — no oneOf/anyOf/$ref, every
// property required, nulls spelled as a type union.
#[must_use]
pub fn crm_intent_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "disposition": {
                "type": "string",
                "enum": ["opportunity", "existing_relationship", "internal", "noise"]
            },
            "lead_title": { "type": ["string", "null"] },
            "contact_name": { "type": ["string", "null"] },
            "company_name": { "type": ["string", "null"] },
            "note_summary": { "type": "string" },
            "tasks": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string" },
                        "due_date": { "type": ["string", "null"] },
                        "detail": { "type": "string" }
                    },
                    "required": ["title", "due_date", "detail"],
                    "additionalProperties": false
                }
            },
            "confidence": { "type": "number" }
        },
        "required": [
            "disposition", "lead_title", "contact_name", "company_name",
            "note_summary", "tasks", "confidence"
        ],
        "additionalProperties": false
    })
}
