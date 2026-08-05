//! The `salesforce` CLI's pure helpers.
//!
//! `apply` assigns the permission set to everyone the platform database knows
//! plus everyone named with `--user`. Getting that list wrong either locks an
//! operator out of the org or assigns the same person twice, so the merge is
//! extracted from the database call and pinned here.
//!
//! `print_changes` is not covered: it writes to stdout directly and returns
//! nothing, so there is no value to assert on without capturing the process's
//! output.

use systemprompt_cli_salesforce::cli::default_spec_path;
use systemprompt_cli_salesforce::commands::apply::{db_unreachable_note, merge_assignees};
use systemprompt_web_admin::salesforce_org::spec::SPEC_RELATIVE_PATH;

#[test]
fn default_spec_path_points_into_services() {
    let path = default_spec_path();
    assert_eq!(path, format!("services/{SPEC_RELATIVE_PATH}"));
    assert!(
        path.starts_with("services/"),
        "the spec must live under services/, got {path}"
    );
}

#[test]
fn merge_sorts_and_deduplicates_across_both_sources() {
    let merged = merge_assignees(
        vec!["zoe@example.com".to_owned(), "amy@example.com".to_owned()],
        vec!["amy@example.com".to_owned(), "bob@example.com".to_owned()],
    );
    assert_eq!(
        merged,
        [
            "amy@example.com".to_owned(),
            "bob@example.com".to_owned(),
            "zoe@example.com".to_owned()
        ]
    );
}

#[test]
fn merge_keeps_the_bootstrap_user_when_the_database_is_empty() {
    let merged = merge_assignees(Vec::new(), vec!["me@example.com".to_owned()]);
    assert_eq!(merged, ["me@example.com".to_owned()]);
}

#[test]
fn merge_with_no_extra_users_returns_the_database_list() {
    let merged = merge_assignees(
        vec!["b@example.com".to_owned(), "a@example.com".to_owned()],
        Vec::new(),
    );
    assert_eq!(
        merged,
        ["a@example.com".to_owned(), "b@example.com".to_owned()]
    );
}

#[test]
fn the_unreachable_note_names_the_cause_and_the_remedy() {
    let note = db_unreachable_note("connection refused");
    assert!(
        note.contains("connection refused"),
        "the underlying error must reach the operator: {note}"
    );
    assert!(
        note.contains("--user"),
        "the note must say which users were assigned: {note}"
    );
    assert!(
        note.contains("re-run"),
        "the note must say what to do next: {note}"
    );
}
