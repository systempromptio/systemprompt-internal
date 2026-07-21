//! Entry point for the Astound Digital binary.
//!
//! Thin by design: every capability is registered at compile time by the
//! extension crates under `extensions/`, and this delegates to the published
//! `systemprompt` core runtime.

use systemprompt_astound as _;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Box::pin(systemprompt_astound::cli::run()).await
}
