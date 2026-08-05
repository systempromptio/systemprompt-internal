//! Command-line surface: the clap types and the default spec location.
//!
//! Shape only. Every subcommand's behaviour lives in
//! [`crate::commands`].

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use systemprompt_web_admin::salesforce_org::spec::SPEC_RELATIVE_PATH;

#[derive(Debug, Parser)]
#[command(
    name = "salesforce",
    about = "Export, diff and apply Salesforce org configuration as code",
    long_about = "Reads SF_TARGET_MY_DOMAIN, SF_TARGET_CONSUMER_KEY, SF_TARGET_JWT_SUBJECT \
                  and SF_TARGET_PRIVATE_KEY to authenticate via the RFC 7523 JWT-bearer grant."
)]
/// The parsed command line: a spec path plus one subcommand.
pub struct Cli {
    #[arg(
        long,
        global = true,
        default_value_t = default_spec_path(),
        help = "Path to the desired-state spec."
    )]
    pub spec: String,

    #[command(subcommand)]
    pub command: Command,
}

/// What to do with the target org.
#[derive(Debug, Subcommand)]
pub enum Command {
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

/// Where the committed spec lives, relative to the repository root.
pub fn default_spec_path() -> String {
    format!("services/{SPEC_RELATIVE_PATH}")
}
