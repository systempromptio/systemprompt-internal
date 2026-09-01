//! Pure prompt-and-parse layer for the categorization job: the prompt, the
//! schema the provider is made to honour, and the validation that turns the
//! provider's answer into a typed value — or refuses it.
//!
//! One prompt produces both the category and the `crm_intent` the proposal
//! job plans from. The schema is derived from [`Categorization`] itself, so
//! the struct is the contract; the response is validated against that same
//! schema by core's [`StructuredOutputProcessor`] and only then deserialized.
//! Nothing is coerced: an off-enum category or a missing field is a failure
//! the job records and retries, never a value it quietly repairs.

use schemars::JsonSchema;
use serde::Deserialize;
use systemprompt::ai::services::structured_output::StructuredOutputProcessor;
use systemprompt::models::ai::{ResponseFormat, StructuredOutputOptions};
pub use systemprompt_mcp_knowledge_bank::proposal::intent::CATEGORIES;
use systemprompt_mcp_knowledge_bank::proposal::intent::{
    CrmIntent, Entity, StructuredSummary, strict_schema,
};

// Why: prompts past this length only add cost — the categorization signal
// lives in the subject and the opening of the body.
const MAX_PROMPT_CONTENT_CHARS: usize = 12_000;

pub const SCHEMA_NAME: &str = "knowledge_categorization";

/// The shape the model must return; [`response_schema`] is derived from it.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct Categorization {
    pub category: Category,
    pub summary: String,
    pub entities: Vec<Entity>,
    pub action_items: Vec<String>,
    pub crm_intent: CrmIntent,
}

/// The closed category set, as a type so the schema's `enum` and the
/// deserializer agree by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Sales,
    Client,
    Product,
    Operations,
    Finance,
    Legal,
    Technical,
    Recruiting,
    Newsletter,
    Notification,
    Spam,
    Other,
}

impl Category {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sales => "sales",
            Self::Client => "client",
            Self::Product => "product",
            Self::Operations => "operations",
            Self::Finance => "finance",
            Self::Legal => "legal",
            Self::Technical => "technical",
            Self::Recruiting => "recruiting",
            Self::Newsletter => "newsletter",
            Self::Notification => "notification",
            Self::Spam => "spam",
            Self::Other => "other",
        }
    }
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

#[must_use]
pub fn response_schema() -> serde_json::Value {
    strict_schema::<Categorization>()
}

#[must_use]
pub fn response_format() -> ResponseFormat {
    ResponseFormat::JsonSchema {
        schema: response_schema(),
        name: Some(SCHEMA_NAME.to_owned()),
        strict: Some(true),
    }
}

#[must_use]
pub fn structured_output_options() -> StructuredOutputOptions {
    StructuredOutputOptions {
        response_format: Some(response_format()),
        validate_schema: Some(true),
        ..StructuredOutputOptions::default()
    }
}

// Why: the provider was told to honour the schema; this is where that claim
// is checked. Core's processor extracts the JSON and validates it strictly
// against the very schema the provider was given, so a violation names the
// path that broke rather than surfacing as a serde error three layers down.
pub fn parse_output(raw: &str) -> Result<Categorization, String> {
    let value = StructuredOutputProcessor::process_response(
        raw,
        &response_format(),
        &structured_output_options(),
    )
    .map_err(|e| format!("response violates the categorization schema: {e}"))?;
    serde_json::from_value(value)
        .map_err(|e| format!("validated response did not deserialize: {e}"))
}

#[must_use]
pub fn structured_json(c: &Categorization) -> serde_json::Value {
    let summary = StructuredSummary {
        summary: c.summary.clone(),
        entities: c.entities.clone(),
        action_items: c.action_items.clone(),
        crm_intent: Some(c.crm_intent.clone()),
    };
    // Why: `structured` is a JSONB column; the typed shape is serialised at
    // the SQL boundary and never hand-assembled.
    serde_json::to_value(summary).unwrap_or(serde_json::Value::Null)
}
