//! Pure prompt-and-parse layer for the categorization job: builds the
//! LLM prompt and JSON schema, and turns model output into the structured
//! value written back to `knowledge_documents`. No AI calls, no database.
//!
//! One prompt produces both the category and the `crm_intent` the proposal
//! job plans from — a second prompt per email would double the spend for no
//! information the first call did not already have.

use serde::Deserialize;
pub use systemprompt_mcp_knowledge_bank::proposal::intent::CATEGORIES;
use systemprompt_mcp_knowledge_bank::proposal::intent::{
    CrmIntent, Entity, StructuredSummary, crm_intent_schema,
};

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
    #[serde(default)]
    pub crm_intent: Option<CrmIntent>,
}

#[must_use]
pub fn system_prompt() -> String {
    format!(
        "You are a knowledge-bank categorization pipeline. You receive one \
         captured document (usually an email) and return structured JSON: a \
         category from this exact set [{}], a 2-3 sentence factual summary, \
         the named entities (people, companies, products), any concrete \
         action items, and a crm_intent object describing what the email is \
         to the business. crm_intent.disposition is one of: opportunity (a \
         prospect or customer asking for something we could sell or quote), \
         existing_relationship (correspondence with a known customer, partner \
         or supplier that is not a new opportunity), internal (from a \
         colleague or our own systems), noise (marketing, notifications, \
         spam). lead_title is a short CRM opportunity title or null; \
         contact_name and company_name are the sender's, or null; \
         note_summary is one paragraph a salesperson would want logged on the \
         record; tasks are concrete follow-ups with an ISO date (YYYY-MM-DD) \
         or null; confidence is 0-1. Never invent facts that are not in the \
         document. Respond with JSON only.",
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

// Why: hand-built rather than derived so it stays inside the subset every
// provider's strict mode accepts — no oneOf/anyOf/$ref, every property
// required, additionalProperties false.
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
            },
            "crm_intent": crm_intent_schema()
        },
        "required": ["category", "summary", "entities", "action_items", "crm_intent"],
        "additionalProperties": false
    })
}

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

#[must_use]
pub fn structured_json(c: &Categorization) -> serde_json::Value {
    let summary = StructuredSummary {
        summary: c.summary.clone(),
        entities: c.entities.clone(),
        action_items: c.action_items.clone(),
        crm_intent: c.crm_intent.clone(),
    };
    // Why: `structured` is a JSONB column; the typed shape is serialised at
    // the SQL boundary and never hand-assembled.
    serde_json::to_value(summary).unwrap_or(serde_json::Value::Null)
}
