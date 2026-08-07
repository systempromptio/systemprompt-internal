//! Recognising a model whose Odoo app is not installed.
//!
//! This mapping earns its place by what it prevents. Odoo answers a call
//! against an uninstalled app with `Object calendar.event doesn't exist`, which
//! a model reads as "the record is missing" and responds to by retrying or by
//! telling the user their meeting was not found. The truth — that this
//! deployment cannot answer at all until an administrator installs Calendar —
//! is not recoverable from the raw fault.
//!
//! The risk in the other direction is worse: an access-rule refusal also names
//! the model, and translating one of those into "the app is not installed"
//! would send someone to reinstall a working module.

use systemprompt_mcp_odoo::apps::{app_for_model, map_missing_app};
use systemprompt_mcp_odoo::error::OdooError;

#[test]
fn models_map_to_the_app_an_administrator_would_search_for() {
    assert_eq!(app_for_model("calendar.event"), Some("Calendar"));
    assert_eq!(app_for_model("project.task"), Some("Project"));
    assert_eq!(app_for_model("project.project"), Some("Project"));
    assert_eq!(app_for_model("discuss.channel"), Some("Discuss"));
    assert_eq!(app_for_model("crm.lead"), Some("CRM"));
}

#[test]
fn a_base_model_belongs_to_no_optional_app() {
    assert_eq!(
        app_for_model("res.partner"),
        None,
        "res.partner ships with Odoo itself; there is no app to blame"
    );
}

#[test]
fn a_missing_model_fault_names_the_app_and_the_way_out() {
    let fault = OdooError::Odoo("Object calendar.event doesn't exist".to_owned());

    let mapped = map_missing_app("calendar.event", fault);

    let OdooError::AppMissing(message) = mapped else {
        panic!("expected AppMissing, got {mapped:?}");
    };
    assert!(message.contains("Calendar"), "name the app: {message}");
    assert!(
        message.contains("not installed"),
        "say what is wrong with it: {message}"
    );
    assert!(
        message.contains("administrator"),
        "and who can fix it: {message}"
    );
}

#[test]
fn a_keyerror_shape_is_recognised_too() {
    let fault = OdooError::Odoo("KeyError: 'project.task'".to_owned());

    assert!(
        matches!(
            map_missing_app("project.task", fault),
            OdooError::AppMissing(_)
        ),
        "the registry lookup failure reaches the boundary in this shape"
    );
}

#[test]
fn an_access_refusal_is_left_alone_even_though_it_names_the_model() {
    let fault = OdooError::Odoo(
        "You are not allowed to access 'calendar.event' records. Contact your administrator."
            .to_owned(),
    );

    let mapped = map_missing_app("calendar.event", fault);

    assert!(
        matches!(&mapped, OdooError::Odoo(m) if m.contains("not allowed")),
        "a permission problem must not be reported as a missing app: {mapped:?}"
    );
}

#[test]
fn unrelated_error_variants_pass_through_untouched() {
    let transport = OdooError::Transport("connection refused".to_owned());

    assert!(matches!(
        map_missing_app("calendar.event", transport),
        OdooError::Transport(_)
    ));
}

#[test]
fn an_unknown_model_still_gets_a_usable_message() {
    let fault = OdooError::Odoo("Object widget.thing doesn't exist".to_owned());

    let OdooError::AppMissing(message) = map_missing_app("widget.thing", fault) else {
        panic!("expected AppMissing");
    };
    assert!(
        message.contains("widget.thing"),
        "with no app name to give, name the model: {message}"
    );
}
