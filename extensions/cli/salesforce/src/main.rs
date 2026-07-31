//! `systemprompt plugins run salesforce` — Salesforce org configuration as
//! code.
//!
//! Argument parsing only. Everything substantive lives in
//! `systemprompt_web_admin::salesforce_org`, next to the rest of the Salesforce
//! code it reuses (JWT-bearer token minting in particular).
//!
//! Credentials come from `SF_TARGET_*` environment variables so the same binary
//! can target any org, including one this deployment has never talked to.

// Why: stdout is this binary's entire interface — it is a CLI, and the
// workspace lints deny printing by default because most crates here are
// libraries.
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "CLI binary: stdout and stderr are the user-facing output"
)]

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use systemprompt_web_admin::salesforce_org::apply::{self, ApplyReport};
use systemprompt_web_admin::salesforce_org::diff::{self, ChangeKind};
use systemprompt_web_admin::salesforce_org::spec::SPEC_RELATIVE_PATH;
use systemprompt_web_admin::salesforce_org::{Connection, OrgSpec, TargetOrg, export};

#[derive(Parser)]
#[command(
    name = "salesforce",
    about = "Export, diff and apply Salesforce org configuration as code",
    long_about = "Reads SF_TARGET_MY_DOMAIN, SF_TARGET_CONSUMER_KEY, SF_TARGET_JWT_SUBJECT \
                  and SF_TARGET_PRIVATE_KEY to authenticate via the RFC 7523 JWT-bearer grant."
)]
struct Cli {
    /// Path to the desired-state spec.
    #[arg(long, global = true, default_value_t = default_spec_path())]
    spec: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Read the target org and print it as an org spec.
    Export {
        /// Write to this file instead of stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Compare the target org against the spec.
    Diff {
        /// Exit non-zero when the org has drifted, for CI gating.
        #[arg(long)]
        exit_code: bool,
    },
    /// Make the target org match the spec.
    Apply {
        /// Validate everything and write nothing. Salesforce still runs a full
        /// metadata validation, so this catches real errors.
        #[arg(long)]
        dry_run: bool,
    },
}

fn default_spec_path() -> String {
    format!("services/{SPEC_RELATIVE_PATH}")
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("error: could not start async runtime: {e}");
            return ExitCode::FAILURE;
        },
    };

    match runtime.block_on(run(Cli::parse())) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        },
    }
}

async fn run(cli: Cli) -> Result<ExitCode, String> {
    let spec_path = PathBuf::from(&cli.spec);
    let target = TargetOrg::from_env().map_err(|e| e.to_string())?;
    let conn = Connection::connect(&target)
        .await
        .map_err(|e| format!("could not authenticate to {}: {e}", target.my_domain))?;

    match cli.command {
        Command::Export { out } => {
            // Why: the committed spec supplies the fields no API can read back.
            let baseline = OrgSpec::load(&spec_path).ok();
            if baseline.is_none() {
                eprintln!(
                    "note: no spec at {}; write-only fields will be placeholders",
                    spec_path.display()
                );
            }
            let exported = export::export_org(&conn, baseline.as_ref())
                .await
                .map_err(|e| e.to_string())?;
            let yaml = exported.to_yaml().map_err(|e| e.to_string())?;
            match out {
                Some(path) => {
                    std::fs::write(&path, yaml)
                        .map_err(|e| format!("could not write {}: {e}", path.display()))?;
                    eprintln!("wrote {}", path.display());
                },
                None => println!("{yaml}"),
            }
            Ok(ExitCode::SUCCESS)
        },

        Command::Diff { exit_code } => {
            let desired = OrgSpec::load(&spec_path).map_err(|e| e.to_string())?;
            let actual = export::export_org(&conn, Some(&desired))
                .await
                .map_err(|e| e.to_string())?;
            let changes = diff::diff(&actual, &desired);
            print_changes(&changes);
            Ok(if exit_code && !changes.is_clean() {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            })
        },

        Command::Apply { dry_run } => {
            let desired = OrgSpec::load(&spec_path).map_err(|e| e.to_string())?;
            let actual = export::export_org(&conn, Some(&desired))
                .await
                .map_err(|e| e.to_string())?;
            let changes = diff::diff(&actual, &desired);
            print_changes(&changes);

            println!(
                "\n{} {}",
                if dry_run { "Validating" } else { "Applying" },
                target.my_domain
            );

            let deploy = apply::apply_metadata(&conn, &desired, dry_run)
                .await
                .map_err(|e| e.to_string())?;
            if !deploy.success {
                println!("  metadata deploy {}: FAILED", deploy.id);
                for line in deploy.failure_lines() {
                    println!("    {line}");
                }
                if let Some(message) = &deploy.error_message {
                    println!("    {message}");
                }
                return Ok(ExitCode::FAILURE);
            }
            println!("  metadata deploy {}: {}", deploy.id, deploy.status);

            let mut report = ApplyReport {
                deploy: Some(deploy),
                ..ApplyReport::default()
            };
            if dry_run {
                println!("  permission sets: skipped (dry run)");
            } else {
                apply::apply_permission_sets(&conn, &desired, &mut report)
                    .await
                    .map_err(|e| e.to_string())?;
                for name in &report.permission_sets_created {
                    println!("  created permission set {name}");
                }
                for grant in &report.app_grants_created {
                    println!("  granted app access {grant}");
                }
                if report.permission_sets_created.is_empty() && report.app_grants_created.is_empty()
                {
                    println!("  permission sets: already correct");
                }
            }

            apply::note_manual_steps(&desired, &mut report);
            if !report.manual_followups.is_empty() {
                println!("\nManual steps (no API exists for these):");
                for note in &report.manual_followups {
                    println!("  - {note}");
                }
            }
            Ok(ExitCode::SUCCESS)
        },
    }
}

fn print_changes(changes: &diff::ChangeSet) {
    let drift = changes.drift();
    if drift.is_empty() {
        println!("No drift: the org matches the spec on every readable field.");
    } else {
        println!("Drift ({}):", drift.len());
        for change in drift {
            println!("{change}");
        }
    }

    let always: Vec<_> = changes
        .changes
        .iter()
        .filter(|c| c.kind == ChangeKind::AlwaysApplied)
        .collect();
    if !always.is_empty() {
        println!("\nAlways applied (not readable from any API, so never compared):");
        for change in always {
            println!("{change}");
        }
    }
}
