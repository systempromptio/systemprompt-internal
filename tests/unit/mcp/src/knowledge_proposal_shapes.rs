//! The wire shapes around a proposal: the sender split, the chatter HTML, the
//! `crm_intent` schema staying inside every provider's strict subset, the
//! status enum, the tagged action encoding, and the call id that keys the
//! approval row.

use systemprompt::identifiers::UserId;
use systemprompt_mcp_knowledge_bank::proposal::approval::proposal_call_id;
use systemprompt_mcp_knowledge_bank::proposal::body::{BodySource, MAX_BODY_CHARS, chatter_body};
use systemprompt_mcp_knowledge_bank::proposal::intent::{
    CrmIntent, DealStageHint, Disposition, StructuredSummary, crm_intent_schema,
};
use systemprompt_mcp_knowledge_bank::proposal::sender::parse_mailbox;
use systemprompt_mcp_knowledge_bank::proposal::{
    ActionTarget, DocumentStatus, OdooAction, Proposal, Sender,
};
use uuid::Uuid;

#[test]
fn parse_mailbox_splits_name_and_address_and_lowercases_the_address() {
    let s = parse_mailbox("\"Victor Acme\" <Victor@Acme.Example>, other@x.y").expect("parsed");
    assert_eq!(s.name.as_deref(), Some("Victor Acme"));
    assert_eq!(s.email, "victor@acme.example");
    assert_eq!(s.display(), "Victor Acme <victor@acme.example>");

    let bare = parse_mailbox("ops@acme.example").expect("bare address");
    assert!(bare.name.is_none());
    assert_eq!(bare.email, "ops@acme.example");

    assert!(parse_mailbox("").is_none());
    assert!(parse_mailbox("Not An Address").is_none());
    assert!(parse_mailbox("Empty <>").is_none());
}

fn sender() -> Sender {
    Sender {
        name: Some("Victor".to_owned()),
        email: "victor@acme.example".to_owned(),
    }
}

#[test]
fn chatter_body_escapes_html_and_drops_the_ingest_header_block() {
    let html = chatter_body(&BodySource {
        sender: &sender(),
        subject: "Pricing <urgent>",
        received: "2026-09-01T09:00:00Z",
        rfc5322_id: "<abc@acme.example>",
        content: "From: Victor <victor@acme.example>\nTo: brain@systemprompt.io\n\nHello & <b>welcome</b>\nline two",
        document_id: "doc-1",
    });
    assert!(html.contains("Pricing &lt;urgent&gt;"));
    assert!(html.contains("Hello &amp; &lt;b&gt;welcome&lt;/b&gt;<br>line two"));
    assert!(
        !html.contains("To: brain@"),
        "the ingest header block is re-rendered, not copied"
    );
    assert!(html.contains("&lt;abc@acme.example&gt;"));
    assert!(html.contains("doc-1"));
}

#[test]
fn chatter_body_is_bounded() {
    let content = "x".repeat(MAX_BODY_CHARS * 3);
    let html = chatter_body(&BodySource {
        sender: &sender(),
        subject: "big",
        received: "",
        rfc5322_id: "<big@x>",
        content: &content,
        document_id: "doc-2",
    });
    assert!(html.len() < MAX_BODY_CHARS + 1024);
    assert!(html.contains("[truncated]"));
}

fn walk(value: &serde_json::Value, banned: &[&str], found: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                if banned.contains(&k.as_str()) {
                    found.push(k.clone());
                }
                walk(v, banned, found);
            }
        },
        serde_json::Value::Array(items) => items.iter().for_each(|v| walk(v, banned, found)),
        _ => {},
    }
}

#[test]
fn crm_intent_schema_stays_inside_the_strict_subset() {
    let schema = crm_intent_schema();
    let mut found = Vec::new();
    walk(
        &schema,
        &["oneOf", "anyOf", "allOf", "$ref", "not"],
        &mut found,
    );
    assert!(found.is_empty(), "banned keywords: {found:?}");
    let props: Vec<&str> = schema["properties"]
        .as_object()
        .expect("properties")
        .keys()
        .map(String::as_str)
        .collect();
    let required: Vec<&str> = schema["required"]
        .as_array()
        .expect("required")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    for p in props {
        assert!(required.contains(&p), "{p} must be required in strict mode");
    }
    assert_eq!(schema["additionalProperties"], false);
}

#[test]
fn a_nullable_closed_enum_is_a_type_list_plus_enum_on_the_wire() {
    let schema = crm_intent_schema();
    let hint = &schema["properties"]["deal_stage_hint"];
    assert_eq!(hint["type"], serde_json::json!(["string", "null"]));
    let values: Vec<&str> = hint["enum"]
        .as_array()
        .expect("enum")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_eq!(
        values,
        ["new", "qualified", "proposition", "won", "lost"],
        "the enum stays closed; null comes from the type list, not a variant"
    );
    assert_eq!(
        schema["properties"]["expected_revenue"]["type"],
        serde_json::json!(["number", "null"])
    );
    assert_eq!(
        schema["properties"]["tasks"]["items"]["properties"]["assignee"]["type"],
        serde_json::json!(["string", "null"])
    );

    let base = serde_json::json!({
        "disposition": "opportunity", "lead_title": null, "contact_name": null,
        "company_name": null, "note_summary": "x", "tasks": [], "confidence": 0.5
    });
    let mut off = base.clone();
    off["deal_stage_hint"] = serde_json::json!("closed-won");
    assert!(
        serde_json::from_value::<CrmIntent>(off).is_err(),
        "an off-enum stage hint is refused, not coerced"
    );
    let mut on = base.clone();
    on["deal_stage_hint"] = serde_json::json!("won");
    let parsed: CrmIntent = serde_json::from_value(on).expect("parsed");
    assert_eq!(parsed.deal_stage_hint, Some(DealStageHint::Won));
    assert_eq!(DealStageHint::Won.odoo_stage_name(), "Won");
    let legacy: CrmIntent = serde_json::from_value(base).expect("pre-deal-field rows still parse");
    assert!(legacy.deal_stage_hint.is_none() && legacy.expected_revenue.is_none());
}

#[test]
fn an_unknown_disposition_is_refused_and_structured_tolerates_missing_intent() {
    let unknown = serde_json::from_value::<CrmIntent>(serde_json::json!({
        "disposition": "galactic", "lead_title": null, "contact_name": null,
        "company_name": null, "note_summary": "x", "tasks": [], "confidence": 0.1
    }));
    assert!(
        unknown.is_err(),
        "the enum is closed; nothing is coerced to noise"
    );
    let known: CrmIntent = serde_json::from_value(serde_json::json!({
        "disposition": "noise", "lead_title": null, "contact_name": null,
        "company_name": null, "note_summary": "x", "tasks": [], "confidence": 0.1
    }))
    .expect("parsed");
    assert_eq!(known.disposition, Disposition::Noise);

    let legacy: StructuredSummary = serde_json::from_value(serde_json::json!({
        "summary": "s", "entities": [], "action_items": []
    }))
    .expect("pre-intent rows still parse");
    assert!(legacy.crm_intent.is_none());
}

#[test]
fn document_status_round_trips_through_its_strings() {
    for status in DocumentStatus::ALL {
        assert_eq!(DocumentStatus::parse(status.as_str()), Some(status));
        let json = serde_json::to_value(status).expect("serializes");
        assert_eq!(json, status.as_str());
    }
    assert!(DocumentStatus::parse("bogus").is_none());
}

fn proposal(revision: i32) -> Proposal {
    Proposal {
        revision,
        sender: sender(),
        actions: vec![
            OdooAction::CreateLead {
                title: "Acme".to_owned(),
                contact_name: None,
                partner_name: None,
                email_from: "victor@acme.example".to_owned(),
                partner_id: None,
                description: String::new(),
                stage_hint: Some("New".to_owned()),
                date_deadline: Some("2026-10-01".to_owned()),
                expected_revenue: Some(1500.0),
                tags: vec!["Sales".to_owned()],
            },
            OdooAction::PostChatter {
                target: ActionTarget::CreatedLead { action_index: 0 },
                subject: "Pricing".to_owned(),
            },
            OdooAction::TagRecord {
                target: ActionTarget::Existing {
                    model: "res.partner".to_owned(),
                    res_id: 7,
                    label: "contact Acme".to_owned(),
                },
                tag: "Sales".to_owned(),
            },
        ],
    }
}

#[test]
fn actions_are_internally_tagged_by_kind_on_the_wire() {
    let json = serde_json::to_value(proposal(1)).expect("serializes");
    assert_eq!(json["actions"][0]["kind"], "create_lead");
    assert_eq!(json["actions"][1]["kind"], "post_chatter");
    assert_eq!(json["actions"][1]["target"]["kind"], "created_lead");
    assert_eq!(json["actions"][1]["target"]["action_index"], 0);
    assert_eq!(json["actions"][2]["kind"], "tag_record");
    assert_eq!(json["actions"][2]["tag"], "Sales");
    assert_eq!(json["actions"][0]["tags"], serde_json::json!(["Sales"]));
    assert_eq!(json["actions"][0]["stage_hint"], "New");
    let back: Proposal = serde_json::from_value(json).expect("round trips");
    assert_eq!(back, proposal(1));
    assert_eq!(back.actions[2].kind(), "tag_record");
    assert_eq!(back.actions[2].depends_on(), None);

    let stored = serde_json::json!({
        "kind": "create_lead", "title": "Old", "contact_name": null, "partner_name": null,
        "email_from": "v@acme.example", "partner_id": null, "description": ""
    });
    let old: OdooAction =
        serde_json::from_value(stored).expect("rows proposed before the deal fields still load");
    assert!(
        matches!(old, OdooAction::CreateLead { tags, stage_hint: None, .. } if tags.is_empty())
    );
}

#[test]
fn the_call_id_is_stable_for_one_revision_and_moves_with_the_next() {
    let owner = UserId::new("admin");
    let id = Uuid::parse_str("00000000-0000-4000-8000-000000000001").expect("uuid");
    let a = proposal_call_id(&owner, id, &proposal(1)).expect("derived");
    let b = proposal_call_id(&owner, id, &proposal(1)).expect("derived");
    let c = proposal_call_id(&owner, id, &proposal(2)).expect("derived");
    assert_eq!(a, b, "same arguments, same row");
    assert_ne!(
        a, c,
        "a re-proposal after expiry must not rejoin the expired row"
    );
    assert_eq!(a.as_str().len(), 64);
}
