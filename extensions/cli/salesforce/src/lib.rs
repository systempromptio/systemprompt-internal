//! Salesforce org configuration as code: the argument surface and the
//! subcommand bodies behind `systemprompt plugins run salesforce`.
//!
//! Everything substantive lives in `systemprompt_web_admin::salesforce_org`,
//! next to the rest of the Salesforce code it reuses (JWT-bearer token minting
//! in particular). This crate is the parser and the printing around it.
//!
//! The binary in `main.rs` is a thin shell over [`cli::Cli`] and
//! [`commands::run`]; keeping them in a library is what lets the parse surface
//! and the pure helpers be tested without spawning a process.

// Why: stdout is this crate's entire interface — it backs a CLI, and the
// workspace lints deny printing by default because most crates here are
// libraries.
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "CLI binary: stdout and stderr are the user-facing output"
)]

pub mod cli;
pub mod commands;
