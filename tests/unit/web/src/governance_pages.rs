//! Structural invariants of the three governance templates, and the one rule
//! the approval queue derives rather than stores.
//!
//! The pages' own logic is exercised by the contract and Playwright suites.
//! What this file guards is what neither of those catches cheaply: a decision
//! button rendered outside its authorisation guard, a table that lost its
//! empty row, or an expiry that stopped being read as a lapsed decision. All
//! three survive a compile and two of them survive a happy-path click-through.

use chrono::{Duration, Utc};
use serde_json::json;
use systemprompt::identifiers::{CallId, UserId};
use systemprompt_web_admin::repositories::governance::approvals::ApprovalRow;

use crate::support::repo_root;

fn template(name: &str) -> String {
    let path = repo_root().join("storage/files/admin/templates").join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn pending_row(expires_in_minutes: i64) -> ApprovalRow {
    let now = Utc::now();
    ApprovalRow {
        call_id: CallId::new("call-1"),
        tool_name: "write_file".to_owned(),
        server_name: "systemprompt".to_owned(),
        arguments: json!({ "path": "/etc/hosts" }),
        args_digest: "digest".to_owned(),
        requested_by: UserId::new("requester"),
        session_id: None,
        trace_id: None,
        rule: "require_approval:write_file".to_owned(),
        status: "pending".to_owned(),
        approver_id: None,
        approver_username: None,
        decided_at: None,
        decision_note: None,
        expires_at: now + Duration::minutes(expires_in_minutes),
        created_at: now - Duration::minutes(5),
    }
}

#[test]
fn an_unanswered_expiry_reads_as_expired_and_is_no_longer_actionable() {
    let live = pending_row(30);
    assert_eq!(live.effective_status(), "pending");
    assert!(live.is_actionable());

    let lapsed = pending_row(-1);
    assert_eq!(
        lapsed.effective_status(),
        "expired",
        "nobody restamps a row nobody answered, so expiry has to be derived"
    );
    assert!(
        !lapsed.is_actionable(),
        "an expired request must not offer a decision that would release nothing"
    );
}

#[test]
fn a_decided_row_keeps_the_verdict_it_was_given() {
    let mut row = pending_row(30);
    row.status = "denied".to_owned();
    row.decided_at = Some(Utc::now());
    row.expires_at = Utc::now() - Duration::hours(1);
    assert_eq!(
        row.effective_status(),
        "denied",
        "expiry must never overwrite a decision a person actually took"
    );
}

#[test]
fn every_approval_decision_control_sits_inside_the_admin_guard() {
    let source = template("governance-approvals.hbs");
    let mut depth: i32 = 0;
    let mut guarded = 0usize;
    for line in source.lines() {
        if line.contains("{{#if ../can_decide}}") || line.contains("{{#if can_decide}}") {
            depth += 1;
        }
        if line.contains("data-action=\"approve\"") || line.contains("data-action=\"deny\"") {
            assert!(
                depth > 0,
                "a decision control is rendered outside the can_decide guard: {line}"
            );
            guarded += 1;
        }
        if line.contains("{{/if}}") && depth > 0 {
            depth -= 1;
        }
    }
    assert_eq!(guarded, 2, "expected exactly an approve and a deny control");
}

#[test]
fn every_governance_listing_has_an_empty_row_and_a_screen_reader_caption() {
    for name in [
        "governance-warnings.hbs",
        "governance-approvals.hbs",
        "governance-secrets.hbs",
    ] {
        let source = template(name);
        assert!(
            source.contains("sp-table__empty"),
            "{name} lost its empty row, so an empty result renders as a bare header"
        );
        assert!(
            source.contains("caption="),
            "{name} lost the table caption a screen reader announces"
        );
        assert!(
            source.contains("components/breadcrumbs"),
            "{name} lost its breadcrumb trail"
        );
    }
}

#[test]
fn the_governance_page_draws_both_enforcement_planes_and_the_hook_plumbing() {
    let source = template("governance-warnings.hbs");
    for collection in [
        "{{#each decisions}}",
        "{{#each findings}}",
        "{{#each hooks}}",
    ] {
        assert!(
            source.contains(collection),
            "the governance template no longer renders {collection}"
        );
    }
    assert!(
        source.contains("{{#each kpis}}"),
        "the KPI strip spans both planes and is the reason the tabs share one page"
    );
}

#[test]
fn the_reason_and_argument_columns_carry_their_full_text_on_the_title() {
    assert!(
        template("governance-warnings.hbs").contains("title=\"{{this.reason_full}}\""),
        "truncating the reason without the full text on the title loses the reason"
    );
    assert!(
        template("governance-approvals.hbs").contains("title=\"{{this.arguments_full}}\""),
        "an approver who cannot read the arguments is not approving anything"
    );
}

// Why: the decisions table folds a call's evaluations into one row, and the
// chain strip is the whole of what makes that fold lossless. A row without it
// is not a denser log, it is a log with evaluations deleted from it.
#[test]
fn the_decisions_row_carries_every_evaluation_it_folded_in() {
    let source = template("governance-warnings.hbs");
    assert!(
        source.contains("{{#each this.chain}}"),
        "the chain strip is gone; the grouped row now hides the evaluations it folded"
    );
    assert!(
        source.contains("{{this.chain_overflow}}"),
        "a chain longer than the strip must say how many pills it left off"
    );
    assert!(
        source.contains("href=\"{{this.href}}\""),
        "each pill links to that evaluation's own audit detail, or the fold is one-way"
    );
}

// Why: the band is a summary of the log below it, so it must never be the only
// place a denial appears, and it must not draw itself when there is nothing to
// report — an empty red box every day is a box nobody reads.
#[test]
fn the_attention_band_is_conditional_and_never_replaces_the_log() {
    let source = template("governance-warnings.hbs");
    assert!(
        source.contains("{{#if attention}}"),
        "the band must be guarded, or a clean window still draws it"
    );
    assert!(
        source.contains("{{#if is_decisions}}"),
        "the full log is rendered independently of the band"
    );
    let band = source
        .split_once("{{#if attention}}")
        .and_then(|(_, rest)| rest.split_once("{{#if is_decisions}}"))
        .map(|(band, _)| band)
        .unwrap_or_default();
    assert!(
        !band.is_empty() && band.contains("{{#each attention}}"),
        "the band sits above the log and lists the calls that objected"
    );
}

// Why: the user column showed a raw UUID, which named nobody. The id is still
// carried on the title because it is what the operator pastes into a search.
#[test]
fn the_user_column_names_a_person_and_keeps_the_id_on_the_title() {
    let source = template("governance-warnings.hbs");
    assert!(
        source.contains("title=\"{{this.user_id}}\">{{this.user_label}}"),
        "the decisions row must render the display name with the raw id on the title"
    );
}
