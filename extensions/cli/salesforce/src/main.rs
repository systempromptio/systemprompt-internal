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

mod cli;
mod commands;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::Cli;

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

    match runtime.block_on(commands::run(Cli::parse())) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        },
    }
}
