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

use systemprompt_mcp_odoo::apps::{app_for_model, map_access_denied, map_missing_app};
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

// Why: `AccessDenied` is Odoo's *authentication* failure, not its permission
// one. Reading it as "you lack rights" is what sent a stale credential to an
// administrator to be granted access the account already held, so the two
// faults must not collapse into the same advice.
#[test]
fn a_failed_credential_is_not_reported_as_a_rights_problem() {
    let fault = OdooError::Odoo("odoo.exceptions.AccessDenied".to_owned());

    let OdooError::AccessDenied(message) =
        map_access_denied("sales@example.com", "crm.lead", fault)
    else {
        panic!("expected AccessDenied");
    };
    assert!(
        message.contains("sales@example.com"),
        "the account whose credential failed is the one fact the user cannot \
         recover themselves: {message}"
    );
    assert!(
        message.contains("/admin/profile"),
        "the remedy is relinking, not a group change: {message}"
    );
    assert!(
        !message.contains("access rights"),
        "must not send the user to be granted rights they already hold: {message}"
    );
}

#[test]
fn a_genuine_rights_refusal_names_the_app_to_grant() {
    let fault =
        OdooError::Odoo("AccessError: you are not allowed to access this document".to_owned());

    let OdooError::AccessDenied(message) =
        map_access_denied("sales@example.com", "crm.lead", fault)
    else {
        panic!("expected AccessDenied");
    };
    assert!(
        message.contains("CRM"),
        "name the app whose access rights need granting: {message}"
    );
}

#[test]
fn an_ordinary_odoo_fault_is_not_mistaken_for_a_permission_problem() {
    // Why: sending someone to request access rights they already hold is the
    // same failure as the missing-app mapping in reverse.
    let fault = OdooError::Odoo("Object crm.lead doesn't exist".to_owned());

    assert!(matches!(
        map_access_denied("sales@example.com", "crm.lead", fault),
        OdooError::Odoo(_)
    ));
}

#[test]
fn a_denied_call_reaches_the_caller_as_something_they_can_act_on() {
    let denied = OdooError::AccessDenied("ask an administrator".to_owned());
    let mcp_error: rmcp::ErrorData = denied.into();

    assert!(
        mcp_error.message.contains("ask an administrator"),
        "the remedy must survive the conversion rather than being flattened \
         into an opaque internal error: {}",
        mcp_error.message
    );
}

// Why: Odoo puts a formatted traceback in the fault's `data.message`, so a
// permission refusal raised while an authentication error was being handled
// carries both phrases. Classifying on whichever check ran first sent the user
// to relink a credential that had authenticated perfectly well — the remedy
// for the fault Odoo did not raise.
#[test]
fn a_permission_refusal_quoting_access_denied_is_still_a_permission_refusal() {
    let fault = OdooError::Odoo(
        "Traceback (most recent call last):\n  raise AccessDenied()\n\nDuring handling of the \
         above exception, another exception occurred:\n\nodoo.exceptions.AccessError: You are \
         not allowed to access this document"
            .to_owned(),
    );

    let OdooError::AccessDenied(message) =
        map_access_denied("sales@example.com", "crm.lead", fault)
    else {
        panic!("expected AccessDenied");
    };
    assert!(
        message.contains("but refused it access"),
        "a fault naming both must resolve to the rights remedy, not the relink one: {message}"
    );
}

// Why: the counterpart — a bare authentication failure must not be softened
// into a rights problem now that the permission check runs first.
#[test]
fn a_bare_authentication_failure_still_asks_for_a_relink() {
    let fault = OdooError::Odoo("odoo.exceptions.AccessDenied: Access Denied".to_owned());

    let OdooError::AccessDenied(message) =
        map_access_denied("sales@example.com", "crm.lead", fault)
    else {
        panic!("expected AccessDenied");
    };
    assert!(
        message.contains("/admin/profile"),
        "an authentication failure must name the relink remedy: {message}"
    );
}
