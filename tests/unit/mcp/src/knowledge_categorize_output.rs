//! Categorization prompt/parse layer: schema shape, output parsing with its
//! brace-span fallback, the closed-category collapse, and prompt truncation.

use systemprompt_knowledge_jobs::internals::{
    CATEGORIES, parse_output, response_schema, structured_json, system_prompt, user_prompt,
};

#[test]
fn schema_pins_the_closed_category_set() {
    let schema = response_schema();
    let enum_values: Vec<&str> = schema["properties"]["category"]["enum"]
        .as_array()
        .expect("enum array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_eq!(enum_values, CATEGORIES);
    assert!(system_prompt().contains("other"));
}

#[test]
fn schema_requires_crm_intent_and_it_parses_back() {
    let schema = response_schema();
    let required: Vec<&str> = schema["required"]
        .as_array()
        .expect("required")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(required.contains(&"crm_intent"));
    assert_eq!(
        schema["properties"]["crm_intent"]["additionalProperties"],
        false
    );
    assert!(system_prompt().contains("opportunity"));

    let raw = r#"{"category":"sales","summary":"s","entities":[],"action_items":[],
        "crm_intent":{"disposition":"opportunity","lead_title":"Acme","contact_name":"V",
        "company_name":"Acme","note_summary":"n","tasks":[{"title":"t","due_date":null,"detail":"d"}],"confidence":0.8}}"#;
    let c = parse_output(raw).expect("parseable");
    let intent = c.crm_intent.as_ref().expect("intent present");
    assert_eq!(intent.tasks.len(), 1);
    let s = structured_json(&c);
    assert_eq!(s["crm_intent"]["disposition"], "opportunity");
}

#[test]
fn parses_clean_json() {
    let raw = r#"{"category":"client","summary":"A client asks about pricing.","entities":[{"name":"Victor","type":"person"}],"action_items":["Reply with the tier sheet"]}"#;
    let c = parse_output(raw).expect("parseable");
    assert_eq!(c.category, "client");
    assert_eq!(c.entities.len(), 1);
    assert_eq!(c.action_items.len(), 1);
}

#[test]
fn parses_json_wrapped_in_prose() {
    let raw = "Here is the result:\n{\"category\":\"spam\",\"summary\":\"Junk.\",\"entities\":[],\"action_items\":[]}\nDone.";
    let c = parse_output(raw).expect("parseable");
    assert_eq!(c.category, "spam");
}

#[test]
fn unknown_category_collapses_to_other() {
    let raw = r#"{"category":"galactic-affairs","summary":"x","entities":[],"action_items":[]}"#;
    let c = parse_output(raw).expect("parseable");
    assert_eq!(c.category, "other");
}

#[test]
fn garbage_output_is_none() {
    assert!(parse_output("no json here").is_none());
    assert!(parse_output("{broken json").is_none());
}

#[test]
fn structured_json_carries_all_fields() {
    let raw = r#"{"category":"sales","summary":"s","entities":[{"name":"Acme","type":"company"}],"action_items":["a"]}"#;
    let c = parse_output(raw).expect("parseable");
    let s = structured_json(&c);
    assert_eq!(s["summary"], "s");
    assert_eq!(s["entities"][0]["name"], "Acme");
    assert_eq!(s["action_items"][0], "a");
}

#[test]
fn user_prompt_truncates_long_content() {
    let long = "x".repeat(50_000);
    let prompt = user_prompt("t", &long);
    assert!(prompt.len() < 20_000);
    assert!(prompt.starts_with("Title: t"));
}
