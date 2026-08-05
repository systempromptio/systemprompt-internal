//! Config loading is deliberately non-fatal: a broken `services/web/` tree
//! degrades the affected section to "absent" instead of taking the server down,
//! and the result is memoised so a failing load is not retried per request.
//! The skills loader adds its own filters — a directory without a readable,
//! parseable `config.yaml` is skipped, and a disabled skill never reaches the
//! page even though its file parsed fine.

use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use systemprompt_web_site::config_loader::skills::{parse_skill_entries, read_skills_dir};
use systemprompt_web_site::config_loader::{ConfigError, log_and_discard_err};

fn write_skill(root: &Path, dir: &str, yaml: &str) {
    let path = root.join(dir);
    fs::create_dir_all(&path).expect("skill dir");
    fs::write(path.join("config.yaml"), yaml).expect("skill config");
}

fn ok_value() -> Result<Option<String>, ConfigError> {
    Ok(Some("loaded".to_owned()))
}

fn absent() -> Result<Option<String>, ConfigError> {
    Ok(None)
}

fn broken() -> Result<Option<String>, ConfigError> {
    Err(ConfigError::Parse {
        config_name: "navigation.yaml".to_owned(),
        message: "unexpected key".to_owned(),
    })
}

#[test]
fn a_successful_load_is_returned_and_then_memoised() {
    static LOCK: OnceLock<Result<Option<String>, String>> = OnceLock::new();

    assert_eq!(
        log_and_discard_err(&LOCK, ok_value, "test").as_deref(),
        Some("loaded")
    );
    assert_eq!(
        log_and_discard_err(&LOCK, broken, "test").as_deref(),
        Some("loaded"),
        "the memoised value must win over a second initialiser"
    );
}

#[test]
fn an_absent_config_is_none_without_being_an_error() {
    static LOCK: OnceLock<Result<Option<String>, String>> = OnceLock::new();

    assert!(log_and_discard_err(&LOCK, absent, "test").is_none());
}

#[test]
fn a_failed_load_is_discarded_as_none_rather_than_panicking() {
    static LOCK: OnceLock<Result<Option<String>, String>> = OnceLock::new();

    assert!(log_and_discard_err(&LOCK, broken, "test").is_none());
    assert!(
        log_and_discard_err(&LOCK, ok_value, "test").is_none(),
        "a failure is cached too; the section stays absent for the process"
    );
}

#[test]
fn a_missing_skills_directory_is_absent_not_an_error() {
    let temp = tempfile::tempdir().expect("tempdir");
    let result = read_skills_dir(&temp.path().join("nope")).expect("missing dir is not an error");

    assert!(result.is_none());
    assert!(
        read_skills_dir(temp.path())
            .expect("existing dir reads")
            .is_some()
    );
}

#[test]
fn only_enabled_skills_with_a_parseable_config_are_loaded() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();

    write_skill(
        root,
        "good",
        "id: good\nname: Good\ndescription: Loads.\ncategory: Salesforce\n",
    );
    write_skill(
        root,
        "disabled",
        "id: disabled\nname: Disabled\ndescription: Hidden.\nenabled: false\n",
    );
    write_skill(root, "unparseable", "id: [oops\n");
    write_skill(root, "incomplete", "name: No Id\n");
    fs::create_dir_all(root.join("no_config")).expect("bare dir");
    fs::write(root.join("loose.yaml"), "id: loose\n").expect("stray file");

    let entries = read_skills_dir(root)
        .expect("dir reads")
        .expect("dir exists");
    let skills = parse_skill_entries(entries);

    let ids: Vec<&str> = skills.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec!["good"]);
    assert_eq!(skills[0].category.as_deref(), Some("Salesforce"));
}
