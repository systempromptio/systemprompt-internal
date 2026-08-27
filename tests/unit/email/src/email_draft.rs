//! `email_send`'s draft validation and preview.
//!
//! The property under test throughout is that nothing here can be coaxed into
//! sending. `email_send` is the only tool on the instance that reaches outside
//! the company, and its guarantee is structural rather than procedural: the
//! first round cannot send because it has no send path, and the second round
//! sends only on an explicit accept carrying `confirm: true`.

use systemprompt_mcp_email::draft::{APPROVE_KEY, CONFIRM_FIELD, SendEmailInput};

fn valid() -> SendEmailInput {
    SendEmailInput {
        to: vec!["ed@systemprompt.io".to_owned()],
        subject: "Quarterly rollout".to_owned(),
        body: "The rollout is complete.".to_owned(),
        reply_to: None,
        res_model: None,
        res_id: None,
    }
}

#[test]
fn accepts_a_well_formed_draft() {
    assert!(valid().validate().is_ok());
}

#[test]
fn rejects_an_empty_recipient_list() {
    let draft = SendEmailInput {
        to: vec![],
        ..valid()
    };
    assert!(draft.validate().is_err());
}

#[test]
fn rejects_a_blank_subject_or_body() {
    // Whitespace, not emptiness: "   " is what a model produces when it has
    // nothing to say, and it would otherwise reach a human as a blank draft.
    let no_subject = SendEmailInput {
        subject: "   ".to_owned(),
        ..valid()
    };
    assert!(no_subject.validate().is_err());

    let no_body = SendEmailInput {
        body: "\n\t ".to_owned(),
        ..valid()
    };
    assert!(no_body.validate().is_err());
}

#[test]
fn rejects_malformed_addresses() {
    for bad in [
        "not-an-address",
        "no-domain@",
        "@no-local.io",
        "no-dot@localhost",
        "two@at@signs.io",
        "spaces in@address.io",
        "trailing.dot@systemprompt.io.",
    ] {
        let draft = SendEmailInput {
            to: vec![bad.to_owned()],
            ..valid()
        };
        assert!(
            draft.validate().is_err(),
            "{bad:?} should have been rejected"
        );
    }
}

#[test]
fn accepts_a_display_name_form() {
    let draft = SendEmailInput {
        to: vec!["Ed Burton <ed@systemprompt.io>".to_owned()],
        ..valid()
    };
    assert!(draft.validate().is_ok());
}

#[test]
fn rejects_half_an_odoo_anchor() {
    // Silently skipping the write-back would leave the caller believing the
    // CRM record was updated when it was not.
    let model_only = SendEmailInput {
        res_model: Some("crm.lead".to_owned()),
        res_id: None,
        ..valid()
    };
    assert!(model_only.validate().is_err());

    let id_only = SendEmailInput {
        res_model: None,
        res_id: Some(42),
        ..valid()
    };
    assert!(id_only.validate().is_err());
}

#[test]
fn anchor_is_present_only_when_both_halves_are() {
    assert!(valid().anchor().is_none());

    let anchored = SendEmailInput {
        res_model: Some("crm.lead".to_owned()),
        res_id: Some(42),
        ..valid()
    };
    assert_eq!(anchored.anchor(), Some(("crm.lead", 42)));
}

#[test]
fn the_plain_text_rendering_carries_everything_a_human_must_see_to_approve() {
    // A client with no artifact rendering shows only this string. If a field
    // that changes where the mail goes is missing from it, someone is being
    // asked to approve a send they cannot fully see.
    let draft = SendEmailInput {
        to: vec!["oliver@example.com".to_owned()],
        reply_to: Some("ed@systemprompt.io".to_owned()),
        res_model: Some("crm.lead".to_owned()),
        res_id: Some(7),
        ..valid()
    };
    let text = draft.as_plain_text();

    assert!(text.contains("oliver@example.com"));
    assert!(text.contains("ed@systemprompt.io"));
    assert!(text.contains("Quarterly rollout"));
    assert!(text.contains("The rollout is complete."));
    assert!(text.contains("crm.lead #7"));
}

#[test]
fn an_unanchored_draft_says_so_rather_than_staying_silent() {
    let card = serde_json::to_value(valid().preview_card()).expect("card serializes");
    let rendered = card.to_string();
    // "no chatter entry will be written" has to be visible on the card: the
    // absence of a CRM trace is exactly the thing a reviewer would otherwise
    // assume was handled.
    assert!(rendered.contains("Will be logged on"));
    assert!(rendered.contains("no chatter entry will be written"));
}

#[test]
fn the_preview_card_shows_the_message_that_would_be_sent() {
    let draft = SendEmailInput {
        to: vec!["oliver@example.com".to_owned()],
        ..valid()
    };
    let card = serde_json::to_value(draft.preview_card()).expect("card serializes");
    let rendered = card.to_string();

    assert!(rendered.contains("oliver@example.com"));
    assert!(rendered.contains("Quarterly rollout"));
    assert!(rendered.contains("The rollout is complete."));
}

#[test]
fn the_confirmation_request_asks_for_the_confirm_boolean_under_the_agreed_key() {
    // Both halves are a wire contract with the client: the key is what the
    // retry's `inputResponses` is looked up by, and the field is what the
    // send is gated on. A rename on either side silently stops the gate
    // working rather than failing loudly.
    let requests = valid()
        .approval_request()
        .expect("the confirmation schema builds");

    assert_eq!(requests.len(), 1, "exactly one thing is being asked");
    let request = requests.get(APPROVE_KEY).expect("keyed by APPROVE_KEY");

    let rendered = serde_json::to_value(request)
        .expect("the request serializes")
        .to_string();
    assert!(rendered.contains(CONFIRM_FIELD));
    // The draft itself must travel inside the prompt, for the same reason as
    // the plain-text test above.
    assert!(rendered.contains("Quarterly rollout"));
}

mod confirm_gate {
    //! The single function standing between a draft and a real email.
    //!
    //! Everything that is not an explicit accept carrying `confirm: true` must
    //! come back as `Declined` or `NotAsked`. These cases are deliberately
    //! exhaustive rather than representative: each one is a way a client could
    //! plausibly answer, and any of them reading as `Confirmed` would mean an
    //! email leaving the company without a human having said yes.

    use rmcp::model::InputResponses;
    use systemprompt_mcp_email::draft::{APPROVE_KEY, Confirmation, confirmation};

    fn responses(value: serde_json::Value) -> InputResponses {
        let mut map = InputResponses::new();
        map.insert(APPROVE_KEY.to_owned(), value);
        map
    }

    #[test]
    fn no_responses_at_all_is_round_one() {
        assert_eq!(confirmation(None), Confirmation::NotAsked);
    }

    #[test]
    fn responses_that_do_not_answer_our_question_are_round_one() {
        // A retry carrying somebody else's key has not answered what we asked,
        // so asking again is right — treating it as a decline would silently
        // discard a draft the human never saw.
        let mut map = InputResponses::new();
        map.insert(
            "some_other_request".to_owned(),
            serde_json::json!({"action": "accept", "content": {"confirm": true}}),
        );
        assert_eq!(confirmation(Some(&map)), Confirmation::NotAsked);
    }

    #[test]
    fn accept_with_confirm_true_is_the_only_way_to_send() {
        let map = responses(serde_json::json!({
            "action": "accept",
            "content": { "confirm": true }
        }));
        assert_eq!(confirmation(Some(&map)), Confirmation::Confirmed);
    }

    #[test]
    fn every_other_answer_declines() {
        for (label, value) in [
            ("explicit decline", serde_json::json!({"action": "decline"})),
            ("cancel", serde_json::json!({"action": "cancel"})),
            (
                "accepted but confirm false",
                serde_json::json!({"action": "accept", "content": {"confirm": false}}),
            ),
            (
                "accepted with no content at all",
                serde_json::json!({"action": "accept"}),
            ),
            (
                "accepted with content but no confirm field",
                serde_json::json!({"action": "accept", "content": {"something_else": true}}),
            ),
            (
                "confirm present but not a boolean",
                serde_json::json!({"action": "accept", "content": {"confirm": "yes"}}),
            ),
            (
                "declined while carrying confirm true",
                serde_json::json!({"action": "decline", "content": {"confirm": true}}),
            ),
            ("unparseable garbage", serde_json::json!("not an object")),
            ("empty object", serde_json::json!({})),
            ("null", serde_json::json!(null)),
        ] {
            assert_eq!(
                confirmation(Some(&responses(value))),
                Confirmation::Declined,
                "{label} must not send"
            );
        }
    }
}

mod wire_shape {
    //! What round one actually puts on the wire.
    //!
    //! These assert the SEP-2322 shape by serializing it, because the contract
    //! that matters is the JSON a client parses, not the Rust type we built it
    //! from. rmcp owns the transport; what is verified here is that we hand it
    //! the right thing.

    use rmcp::model::InputRequiredResult;
    use systemprompt_mcp_email::draft::{APPROVE_KEY, CONFIRM_FIELD, SendEmailInput};

    fn round_one_result() -> serde_json::Value {
        let draft = SendEmailInput {
            to: vec!["oliver@example.com".to_owned()],
            subject: "Quarterly rollout".to_owned(),
            body: "The rollout is complete.".to_owned(),
            reply_to: None,
            res_model: None,
            res_id: None,
        };
        let result = InputRequiredResult::from_input_requests(
            draft.approval_request().expect("the schema builds"),
        );
        serde_json::to_value(result).expect("the result serializes")
    }

    #[test]
    fn round_one_declares_result_type_input_required() {
        // The discriminator a client switches on. If this is not exactly
        // "input_required", an MRTR-capable client treats round one as a
        // completed call and the draft is never confirmed.
        assert_eq!(round_one_result()["resultType"], "input_required");
    }

    #[test]
    fn round_one_carries_our_request_under_the_agreed_key() {
        let value = round_one_result();
        let requests = value["inputRequests"]
            .as_object()
            .expect("inputRequests is an object");

        assert_eq!(requests.len(), 1);
        assert!(
            requests.contains_key(APPROVE_KEY),
            "the client echoes this key back in inputResponses"
        );
    }

    #[test]
    fn the_request_is_an_elicitation_asking_for_the_confirm_boolean() {
        let value = round_one_result();
        let request = &value["inputRequests"][APPROVE_KEY];

        assert_eq!(request["method"], "elicitation/create");
        let schema = &request["params"]["requestedSchema"];
        assert_eq!(schema["properties"][CONFIRM_FIELD]["type"], "boolean");
        assert_eq!(
            schema["required"]
                .as_array()
                .expect("required is an array")[0],
            CONFIRM_FIELD
        );
    }

    #[test]
    fn round_one_shows_the_human_what_they_are_approving() {
        let value = round_one_result();
        let message = value["inputRequests"][APPROVE_KEY]["params"]["message"]
            .as_str()
            .expect("the elicitation carries a message");

        // A client with no artifact rendering shows only this.
        assert!(message.contains("oliver@example.com"));
        assert!(message.contains("Quarterly rollout"));
        assert!(message.contains("The rollout is complete."));
    }

    #[test]
    fn round_one_carries_no_result_of_its_own() {
        // Round one must not look like a completed tool call in any way: no
        // content, no structuredContent. A client that fell back to reading
        // those would see a "successful" send that never happened.
        let value = round_one_result();
        assert!(value.get("content").is_none());
        assert!(value.get("structuredContent").is_none());
    }
}
