//! Pure prompt-and-parse layer for the categorization job: builds the
//! LLM prompt and JSON schema, and turns model output into the structured
//! value written back to `knowledge_documents`. No AI calls, no database.

use serde::Deserialize;

/// Closed category set. `other` is the honest fallback so the model is never
/// forced to mislabel; `spam` lets the later review pass filter noise out.
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

// Why: prompts past this length only add cost — the categorization signal
// lives in the subject and the opening of the body.
const MAX_PROMPT_CONTENT_CHARS: usize = 12_000;

/// The shape the model must return, mirrored by [`response_schema`].
#[derive(Debug, Clone, Deserialize)]
pub struct Categorization {
    pub category: String,
    pub summary: String,
    #[serde(default)]
    pub entities: Vec<Entity>,
    #[serde(default)]
    pub action_items: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct Entity {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
}

#[must_use]
pub fn system_prompt() -> String {
    format!(
        "You are a knowledge-bank categorization pipeline. You receive one \
         captured document (usually an email) and return structured JSON: a \
         category from this exact set [{}], a 2-3 sentence factual summary, \
         the named entities (people, companies, products), and any concrete \
         action items. Never invent facts that are not in the document. \
         Respond with JSON only.",
        CATEGORIES.join(", ")
    )
}

#[must_use]
pub fn user_prompt(title: &str, content: &str) -> String {
    let mut body = content;
    if let Some((idx, _)) = body.char_indices().nth(MAX_PROMPT_CONTENT_CHARS) {
        body = &body[..idx];
    }
    format!("Title: {title}\n\nDocument:\n{body}")
}

#[must_use]
pub fn response_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "category": { "type": "string", "enum": CATEGORIES },
            "summary": { "type": "string" },
            "entities": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "type": { "type": "string" }
                    },
                    "required": ["name", "type"],
                    "additionalProperties": false
                }
            },
            "action_items": {
                "type": "array",
                "items": { "type": "string" }
            }
        },
        "required": ["category", "summary", "entities", "action_items"],
        "additionalProperties": false
    })
}

/// Parse model output into a [`Categorization`].
///
/// Tolerates prose around the JSON (some providers wrap it) by retrying on
/// the outermost brace span; an unknown category collapses to `other` so a
/// creative model cannot widen the closed set.
#[must_use]
pub fn parse_output(raw: &str) -> Option<Categorization> {
    let parsed: Option<Categorization> = serde_json::from_str(raw.trim()).ok().or_else(|| {
        let start = raw.find('{')?;
        let end = raw.rfind('}')?;
        serde_json::from_str(raw.get(start..=end)?).ok()
    });
    let mut categorization = parsed?;
    if !CATEGORIES.contains(&categorization.category.as_str()) {
        "other".clone_into(&mut categorization.category);
    }
    Some(categorization)
}

/// The `structured` JSONB written back to the document row.
#[must_use]
pub fn structured_json(c: &Categorization) -> serde_json::Value {
    serde_json::json!({
        "summary": c.summary,
        "entities": c.entities,
        "action_items": c.action_items,
    })
}
