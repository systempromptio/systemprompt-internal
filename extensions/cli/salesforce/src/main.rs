//! `systemprompt plugins run salesforce` — Salesforce org configuration as
//! code.
//!
//! Process shell only: install logging, start a runtime, hand the parsed
//! arguments to the library. The parse surface and the subcommands live in
//! [`systemprompt_cli_salesforce`].
//!
//! Credentials come from `SF_TARGET_*` environment variables so the same binary
//! can target any org, including one this deployment has never talked to.

// Why: stderr is how this binary reports a failure before the library's
// printing takes over; the workspace lints deny printing by default.
#![allow(
    clippy::print_stderr,
    reason = "CLI binary: stderr is the user-facing error channel"
)]

use std::process::ExitCode;

use clap::Parser;
use systemprompt_cli_salesforce::cli::Cli;
use systemprompt_cli_salesforce::commands;

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
