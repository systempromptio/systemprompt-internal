//! The structured shape the categorization prompt returns, shared by the job
//! that asks for it and the planner that consumes it.
//!
//! The wire schema is *derived from these types* ([`strict_schema`]) and then
//! tightened to the subset every provider enforces natively — Anthropic's
//! forced tool, `OpenAI`'s `strict: true`, Gemini's `responseSchema`: every
//! property required, no additional properties, no `$ref`/`anyOf`, nulls as a
//! type union. The same schema is what the response is validated against
//! before it is deserialized, so "the model returned the struct" is checked,
//! not hoped. `crm_intent` is deliberately flat — no unions — for that reason.
//! Which Odoo records exist is not the model's to know; it states what the
//! email *is*, and the planner decides what that becomes.

use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
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
    Noise,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IntentTask {
    pub title: String,
    pub due_date: Option<String>,
    pub detail: String,
}

// Why: keywords no provider's strict mode accepts, and that carry no
// constraint the validator would enforce anyway.
const DROPPED_KEYWORDS: &[&str] = &[
    "$schema",
    "title",
    "description",
    "format",
    "$defs",
    "definitions",
];

#[must_use]
pub fn crm_intent_schema() -> serde_json::Value {
    strict_schema::<CrmIntent>()
}

// JSON: protocol boundary — the schema handed to the provider and to the
// validator, derived from `T` so it cannot drift from the struct.
#[must_use]
pub fn strict_schema<T: JsonSchema>() -> serde_json::Value {
    let settings = SchemaSettings::draft2020_12().with(|s| s.inline_subschemas = true);
    let mut schema = settings
        .into_generator()
        .into_root_schema_for::<T>()
        .to_value();
    strictify(&mut schema);
    schema
}

fn strictify(schema: &mut serde_json::Value) {
    let Some(object) = schema.as_object_mut() else {
        return;
    };
    for key in DROPPED_KEYWORDS {
        object.remove(*key);
    }
    if let Some(variants) = object.remove("anyOf").or_else(|| object.remove("oneOf")) {
        // Why: `Option<T>` derives as a union with null; strict modes want the
        // null spelled as a type list on the one real schema instead.
        collapse_nullable(object, &variants);
    }
    if object.get("type").and_then(|t| t.as_str()) == Some("object") {
        let keys: Vec<serde_json::Value> = object
            .get("properties")
            .and_then(|p| p.as_object())
            .map(|p| p.keys().cloned().map(serde_json::Value::String).collect())
            .unwrap_or_default();
        object.insert("required".to_owned(), serde_json::Value::Array(keys));
        object.insert(
            "additionalProperties".to_owned(),
            serde_json::Value::Bool(false),
        );
    }
    if let Some(properties) = object.get_mut("properties").and_then(|p| p.as_object_mut()) {
        properties.values_mut().for_each(strictify);
    }
    if let Some(items) = object.get_mut("items") {
        strictify(items);
    }
}

fn collapse_nullable(
    object: &mut serde_json::Map<String, serde_json::Value>,
    variants: &serde_json::Value,
) {
    let Some(variants) = variants.as_array() else {
        return;
    };
    let mut real: Vec<serde_json::Value> = variants
        .iter()
        .filter(|v| v.get("type").and_then(|t| t.as_str()) != Some("null"))
        .cloned()
        .collect();
    let nullable = real.len() < variants.len();
    if real.len() != 1 {
        return;
    }
    let mut inner = real.remove(0);
    strictify(&mut inner);
    if let Some(inner) = inner.as_object() {
        for (k, v) in inner {
            object.insert(k.clone(), v.clone());
        }
    }
    if nullable && let Some(t) = object.get("type").cloned() {
        let mut types = match t {
            serde_json::Value::Array(a) => a,
            other => vec![other],
        };
        types.push(serde_json::Value::String("null".to_owned()));
        object.insert("type".to_owned(), serde_json::Value::Array(types));
    }
}
