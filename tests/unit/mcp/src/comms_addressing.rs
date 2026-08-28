//! Addressing is the whole anti-spam mechanism: the address form decides
//! whether a message may enter a running conversation. `@user` and `#channel`
//! raise an unread count and nothing more; only `@user/handle` can interrupt,
//! and only the session it names.

use systemprompt_mcp_comms::store::{Address, DeliveryClass, classify, parse_address};

#[test]
fn a_bare_user_is_an_inbox_address() {
    assert_eq!(
        parse_address("@ed").ok(),
        Some(Address::User("ed".to_owned()))
    );
}

#[test]
fn a_user_and_handle_addresses_one_session() {
    assert_eq!(
        parse_address("@ed/odoo-crm").ok(),
        Some(Address::Session {
            user: "ed".to_owned(),
            handle: "odoo-crm".to_owned()
        })
    );
}

#[test]
fn a_handle_keeps_the_disambiguating_suffix_and_branch() {
    let parsed = parse_address("@ed/odoo-crm#2").ok();
    assert_eq!(
        parsed,
        Some(Address::Session {
            user: "ed".to_owned(),
            handle: "odoo-crm#2".to_owned()
        })
    );
    assert_eq!(
        parse_address("@ed/core:next").ok(),
        Some(Address::Session {
            user: "ed".to_owned(),
            handle: "core:next".to_owned()
        })
    );
}

#[test]
fn a_channel_is_recognised_by_its_hash() {
    assert_eq!(
        parse_address("#crm").ok(),
        Some(Address::Channel("crm".to_owned()))
    );
}

#[test]
fn addresses_are_case_insensitive() {
    assert_eq!(
        parse_address("@Ed").ok(),
        Some(Address::User("ed".to_owned()))
    );
}

#[test]
fn a_bare_word_is_rejected_rather_than_guessed_at() {
    // Reading `crm` as @crm would deliver a channel post to a person.
    assert!(parse_address("crm").is_err());
    assert!(parse_address("").is_err());
    assert!(parse_address("   ").is_err());
}

#[test]
fn incomplete_addresses_are_rejected() {
    assert!(parse_address("@").is_err());
    assert!(parse_address("#").is_err());
    assert!(parse_address("@ed/").is_err());
    assert!(parse_address("@/handle").is_err());
}

#[test]
fn only_a_live_session_address_interrupts() {
    assert_eq!(classify(true, true, false), DeliveryClass::Session);
}

#[test]
fn a_user_address_never_interrupts_however_live_they_are() {
    assert_eq!(classify(false, true, false), DeliveryClass::Inbox);
    assert_eq!(classify(false, false, false), DeliveryClass::Inbox);
}

#[test]
fn a_session_address_degrades_to_inbox_when_the_target_is_idle() {
    assert_eq!(classify(true, false, false), DeliveryClass::Inbox);
}

#[test]
fn urgent_overrides_every_other_consideration() {
    assert_eq!(classify(false, false, true), DeliveryClass::Urgent);
    assert_eq!(classify(true, true, true), DeliveryClass::Urgent);
}

#[test]
fn delivery_classes_match_the_database_check_constraint() {
    assert_eq!(DeliveryClass::Inbox.as_str(), "inbox");
    assert_eq!(DeliveryClass::Session.as_str(), "session");
    assert_eq!(DeliveryClass::Urgent.as_str(), "urgent");
}
