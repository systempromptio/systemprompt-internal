//! Categorization contract: the wire schema is derived from the Rust type and
//! tightened to the strict subset every provider enforces; the response is
//! validated against that schema before deserialization; nothing off-contract
//! is repaired.

use systemprompt::models::ai::ResponseFormat;
use systemprompt_knowledge_jobs::internals::{
    CATEGORIES, Category, parse_output, response_format, response_schema, structured_json,
    system_prompt, user_prompt,
};

const GOOD: &str = r#"{"category":"sales","summary":"Victor asks for pricing.","entities":[{"name":"Acme","type":"company"}],"action_items":["Send the tier sheet"],
  "crm_intent":{"disposition":"opportunity","lead_title":"Acme — pricing","contact_name":"Victor","company_name":"Acme",
  "note_summary":"Pricing enquiry.","tasks":[{"title":"Send tier sheet","due_date":"2026-09-10","detail":"Enterprise tier"}],"confidence":0.9}}"#;

fn walk(
    value: &serde_json::Value,
    f: &mut dyn FnMut(&str, &serde_json::Map<String, serde_json::Value>),
) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                f(k, map);
                walk(v, f);
            }
        },
        serde_json::Value::Array(items) => items.iter().for_each(|v| walk(v, f)),
        _ => {},
    }
}

#[test]
fn schema_is_derived_and_pins_the_closed_category_set() {
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
fn schema_stays_inside_the_strict_subset_every_provider_enforces() {
    let schema = response_schema();
    let mut banned = Vec::new();
    let mut loose_objects = Vec::new();
    walk(&schema, &mut |key, map| {
        if matches!(
            key,
            "oneOf" | "anyOf" | "allOf" | "$ref" | "$defs" | "definitions" | "format"
        ) {
            banned.push(key.to_owned());
        }
        if key == "type" && map.get("type").and_then(|t| t.as_str()) == Some("object") {
            let props: Vec<&str> = map["properties"]
                .as_object()
                .map(|p| p.keys().map(String::as_str).collect())
                .unwrap_or_default();
            let required: Vec<&str> = map["required"]
                .as_array()
                .map(|r| r.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();
            if map.get("additionalProperties") != Some(&serde_json::Value::Bool(false))
                || props.iter().any(|p| !required.contains(p))
            {
                loose_objects.push(map.clone());
            }
        }
    });
    assert!(banned.is_empty(), "banned keywords present: {banned:?}");
    assert!(
        loose_objects.is_empty(),
        "objects not strict: {loose_objects:#?}"
    );
    assert_eq!(
        schema["properties"]["crm_intent"]["properties"]["lead_title"]["type"],
        serde_json::json!(["string", "null"])
    );
    assert!(matches!(
        response_format(),
        ResponseFormat::JsonSchema {
            strict: Some(true),
            ..
        }
    ));
}

#[test]
fn a_conforming_response_parses_and_round_trips_into_structured() {
    let c = parse_output(GOOD).expect("conforming response");
    assert_eq!(c.category, Category::Sales);
    assert_eq!(c.entities.len(), 1);
    assert_eq!(c.crm_intent.tasks.len(), 1);
    let s = structured_json(&c);
    assert_eq!(s["summary"], "Victor asks for pricing.");
    assert_eq!(s["entities"][0]["name"], "Acme");
    assert_eq!(s["crm_intent"]["disposition"], "opportunity");
}

#[test]
fn an_off_enum_category_is_refused_not_collapsed() {
    let raw = GOOD.replace(
        "\"category\":\"sales\"",
        "\"category\":\"galactic-affairs\"",
    );
    let err = parse_output(&raw).expect_err("off-enum category");
    assert!(err.contains("category"), "names the failing path: {err}");
}

#[test]
fn a_missing_field_or_unknown_disposition_is_refused() {
    let missing = GOOD.replace(",\"confidence\":0.9", "");
    assert!(parse_output(&missing).is_err(), "confidence is required");
    let bad = GOOD.replace(
        "\"disposition\":\"opportunity\"",
        "\"disposition\":\"maybe\"",
    );
    assert!(parse_output(&bad).is_err(), "disposition is a closed enum");
    let extra = GOOD.replacen("{\"category\"", "{\"vibe\":\"good\",\"category\"", 1);
    assert!(
        parse_output(&extra).is_err(),
        "additional properties are refused"
    );
}

#[test]
fn garbage_output_is_an_error() {
    assert!(parse_output("no json here").is_err());
    assert!(parse_output("{broken json").is_err());
}

#[test]
fn user_prompt_is_truncated_to_the_budget() {
    let long = "x".repeat(20_000);
    let prompt = user_prompt("t", &long);
    assert!(prompt.len() < 12_100);
    assert!(prompt.starts_with("Title: t"));
}
