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

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use systemprompt_web_admin::repositories::users::salesforce_identity;
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
        /// Assign the permission set to this Salesforce Username as well as to
        /// everyone in the platform database. Repeatable.
        ///
        /// For bootstrapping: a fresh org has no SSO logins yet, so the
        /// database has nobody to read. Name yourself here or the apply that
        /// flips the app to `AdminApprovedPreAuthorized` locks you out of it.
        #[arg(long = "user", value_name = "USERNAME")]
        users: Vec<String>,
    },
}

fn default_spec_path() -> String {
    format!("services/{SPEC_RELATIVE_PATH}")
}

fn main() -> ExitCode {
    // Why: core's initializer, not a hand-rolled tracing_subscriber. It also
    // writes to stderr — which is what keeps stdout this binary's interface —
    // and it guards itself with a OnceLock. Installing our own subscriber here
    // made the core bootstrap `apply` runs later panic on "a global default
    // trace dispatcher has already been set".
    systemprompt::logging::init_console_logging();

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
        Command::Export { out } => run_export(&conn, &spec_path, out).await,
        Command::Diff { exit_code } => run_diff(&conn, &spec_path, exit_code).await,
        Command::Apply { dry_run, users } => {
            run_apply(&conn, &spec_path, &target, dry_run, users).await
        },
    }
}

async fn run_export(
    conn: &Connection,
    spec_path: &Path,
    out: Option<PathBuf>,
) -> Result<ExitCode, String> {
    // Why: the committed spec supplies the fields no API can read back.
    let baseline = OrgSpec::load(spec_path).ok();
    if baseline.is_none() {
        eprintln!(
            "note: no spec at {}; write-only fields will be placeholders",
            spec_path.display()
        );
    }
    let exported = export::export_org(conn, baseline.as_ref())
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
}

async fn run_diff(
    conn: &Connection,
    spec_path: &Path,
    exit_code: bool,
) -> Result<ExitCode, String> {
    let desired = OrgSpec::load(spec_path).map_err(|e| e.to_string())?;
    let actual = export::export_org(conn, Some(&desired))
        .await
        .map_err(|e| e.to_string())?;
    let changes = diff::diff(&actual, &desired);
    print_changes(&changes);
    Ok(if exit_code && !changes.is_clean() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

async fn run_apply(
    conn: &Connection,
    spec_path: &Path,
    target: &TargetOrg,
    dry_run: bool,
    extra_users: Vec<String>,
) -> Result<ExitCode, String> {
    let my_domain = &target.my_domain;
    let certificate = target.certificate_pem.as_deref();

    // Why: checked before anything runs, including in --dry-run. A deploy
    // without the certificate clears the app's digital signature and breaks the
    // grant this command authenticates with, so refusing early is the whole
    // point — failing at the deploy would leave the writes before it applied.
    apply::check_certificate_present(certificate).map_err(|e| e.to_string())?;

    let desired = OrgSpec::load(spec_path).map_err(|e| e.to_string())?;
    let actual = export::export_org(conn, Some(&desired))
        .await
        .map_err(|e| e.to_string())?;
    print_changes(&diff::diff(&actual, &desired));

    println!(
        "\n{} {my_domain}",
        if dry_run { "Validating" } else { "Applying" }
    );

    let mut report = ApplyReport::default();
    let (assignees, db_note) = collect_assignees(extra_users).await;
    if let Some(note) = db_note {
        report.manual_followups.push(note);
    }

    // Order is load-bearing. Permission sets, grants and assignments all run
    // BEFORE the metadata deploy, because the deploy is what flips the app to
    // AdminApprovedPreAuthorized — from that moment only holders of the
    // permission set can authenticate. Deploying first would open a window in
    // which nobody holds it, including whoever is running this command.
    if dry_run {
        println!("  permission sets, grants and assignments: skipped (dry run)");
        println!("  hosted MCP servers: skipped (dry run)");
    } else {
        apply::apply_permission_sets(conn, &desired, &mut report)
            .await
            .map_err(|e| e.to_string())?;
        apply::apply_assignments(conn, &desired, &assignees, &mut report)
            .await
            .map_err(|e| e.to_string())?;
        report_permission_sets(&report);
        apply::apply_hosted_mcp_servers(conn, &desired, &mut report)
            .await
            .map_err(|e| e.to_string())?;
        report_servers(&report);
    }

    let deploy = apply::apply_metadata(conn, &desired, certificate, dry_run)
        .await
        .map_err(|e| e.to_string())?;
    let failed = !deploy.success;
    if failed {
        println!("  metadata deploy {}: FAILED", deploy.id);
        for line in deploy.failure_lines() {
            println!("    {line}");
        }
        if let Some(message) = &deploy.error_message {
            println!("    {message}");
        }
    } else {
        println!("  metadata deploy {}: {}", deploy.id, deploy.status);
    }
    report.deploy = Some(deploy);

    if !report.manual_followups.is_empty() {
        println!("\nNeeds a human:");
        for note in &report.manual_followups {
            println!("  - {note}");
        }
    }
    Ok(if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// The Salesforce Usernames to assign the permission set to.
///
/// The platform database is the source of truth — `salesforce_user_identities`
/// records a Username for every user who has completed an SSO login — with
/// `--user` layered on top for the bootstrap case, where a fresh org has no
/// logins yet and so nothing to read.
///
/// An unreachable database degrades to a note rather than failing the apply.
/// The metadata half is independently useful, and refusing to configure an org
/// because a Postgres container is down would be the wrong trade.
async fn collect_assignees(extra: Vec<String>) -> (Vec<String>, Option<String>) {
    let (mut names, note) = match load_db_usernames().await {
        Ok(names) => (names, None),
        Err(e) => (
            Vec::new(),
            Some(format!(
                "could not read Salesforce usernames from the platform database ({e}). \
                 Only the --user values were assigned; re-run this apply once the \
                 database is reachable to assign everyone else."
            )),
        ),
    };
    names.extend(extra);
    names.sort();
    names.dedup();
    (names, note)
}

async fn load_db_usernames() -> Result<Vec<String>, String> {
    use systemprompt::config::{ProfileBootstrap, SecretsBootstrap, init_config};
    use systemprompt::system::AppContext;

    ProfileBootstrap::init().map_err(|e| format!("profile: {e}"))?;
    SecretsBootstrap::init().map_err(|e| format!("secrets: {e}"))?;
    init_config().map_err(|e| format!("config: {e}"))?;
    let ctx = AppContext::new()
        .await
        .map_err(|e| format!("app context: {e}"))?;
    let pool = ctx
        .db_pool()
        .write_pool_arc()
        .map_err(|e| format!("write pool: {e}"))?;
    salesforce_identity::list_salesforce_usernames(&pool)
        .await
        .map_err(|e| e.to_string())
}

fn report_servers(report: &ApplyReport) {
    for name in &report.servers_activated {
        println!("  activated hosted MCP server {name}");
    }
    if report.servers_activated.is_empty() {
        println!("  hosted MCP servers: already active");
    }
}

fn report_permission_sets(report: &ApplyReport) {
    for name in &report.permission_sets_created {
        println!("  created permission set {name}");
    }
    for grant in &report.app_grants_created {
        println!("  granted app access {grant}");
    }
    for assignment in &report.assignments_created {
        println!("  assigned {assignment}");
    }
    if report.permission_sets_created.is_empty()
        && report.app_grants_created.is_empty()
        && report.assignments_created.is_empty()
    {
        println!("  permission sets, grants and assignments: already correct");
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
