//! The `salesforce` CLI's argument surface.
//!
//! The binary is invoked through `systemprompt plugins run salesforce`, so a
//! renamed flag or a subcommand that stops taking a value fails at the operator
//! rather than at compile time. These tests pin the surface: the spec default,
//! each subcommand's flags, the repeatable `--user`, and that unknown input is
//! rejected instead of silently ignored.

use clap::{CommandFactory, Parser};
use systemprompt_cli_salesforce::cli::{Cli, Command, default_spec_path};

fn parse(args: &[&str]) -> Cli {
    Cli::try_parse_from(args).expect("arguments should parse")
}

#[test]
fn clap_definition_is_internally_consistent() {
    Cli::command().debug_assert();
}

#[test]
fn spec_defaults_to_the_committed_path() {
    let cli = parse(&["salesforce", "diff"]);
    assert_eq!(cli.spec, default_spec_path());
}

#[test]
fn spec_is_global_so_it_may_follow_the_subcommand() {
    let before = parse(&["salesforce", "--spec", "/tmp/org.yaml", "diff"]);
    let after = parse(&["salesforce", "diff", "--spec", "/tmp/org.yaml"]);
    assert_eq!(before.spec, "/tmp/org.yaml");
    assert_eq!(after.spec, "/tmp/org.yaml");
}

#[test]
fn export_out_is_optional() {
    match parse(&["salesforce", "export"]).command {
        Command::Export { out } => assert!(out.is_none(), "expected stdout export, got {out:?}"),
        other => panic!("expected Export, got {other:?}"),
    }
    match parse(&["salesforce", "export", "--out", "org.yaml"]).command {
        Command::Export { out } => {
            assert_eq!(
                out.expect("--out should be captured").to_str(),
                Some("org.yaml")
            );
        },
        other => panic!("expected Export, got {other:?}"),
    }
}

#[test]
fn diff_exit_code_is_a_flag_that_defaults_off() {
    match parse(&["salesforce", "diff"]).command {
        Command::Diff { exit_code } => assert!(!exit_code),
        other => panic!("expected Diff, got {other:?}"),
    }
    match parse(&["salesforce", "diff", "--exit-code"]).command {
        Command::Diff { exit_code } => assert!(exit_code),
        other => panic!("expected Diff, got {other:?}"),
    }
}

#[test]
fn apply_collects_every_repeated_user() {
    match parse(&[
        "salesforce",
        "apply",
        "--dry-run",
        "--user",
        "one@example.com",
        "--user",
        "two@example.com",
    ])
    .command
    {
        Command::Apply { dry_run, users } => {
            assert!(dry_run);
            assert_eq!(users, ["one@example.com", "two@example.com"]);
        },
        other => panic!("expected Apply, got {other:?}"),
    }
}

#[test]
fn apply_defaults_to_a_real_run_with_no_extra_users() {
    match parse(&["salesforce", "apply"]).command {
        Command::Apply { dry_run, users } => {
            assert!(!dry_run);
            assert!(users.is_empty(), "expected no extra users, got {users:?}");
        },
        other => panic!("expected Apply, got {other:?}"),
    }
}

#[test]
fn bad_input_is_rejected() {
    assert!(
        Cli::try_parse_from(["salesforce"]).is_err(),
        "a subcommand is required"
    );
    assert!(
        Cli::try_parse_from(["salesforce", "deploy"]).is_err(),
        "unknown subcommands must not be accepted"
    );
    assert!(
        Cli::try_parse_from(["salesforce", "diff", "--force"]).is_err(),
        "unknown flags must not be accepted"
    );
    assert!(
        Cli::try_parse_from(["salesforce", "apply", "--user"]).is_err(),
        "--user takes a value"
    );
}
