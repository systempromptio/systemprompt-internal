//! Prove that what `factsheet_get` hands out is what `factsheet_render`
//! accepts.
//!
//! The edit loop depends on this: the tool serialises a `FactsheetDoc` to YAML,
//! the caller changes some blocks, and the result comes back as `doc`. If a
//! field did not survive that trip the sheet would silently lose content.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a developer harness reports to the terminal"
)]

use std::path::PathBuf;
use systemprompt_factsheet::{EnginePaths, FactsheetDoc, FactsheetEngine};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .ok_or("cannot locate repository root")?
        .to_path_buf();
    let engine = FactsheetEngine::new(EnginePaths {
        root: repo.join("storage/files/factsheet"),
        script: repo.join("scripts/factsheet-render.py"),
        python: PathBuf::from("python3"),
    })?;

    let mut failures = 0;
    for id in engine.list_sheets()? {
        let original = engine.load_sheet(&id)?;
        let yaml = serde_yaml::to_string(&original)?;
        let reparsed: FactsheetDoc = serde_yaml::from_str(&yaml)?;

        let a = engine.render_html(&original)?;
        let b = engine.render_html(&reparsed)?;
        if a == b {
            println!("{id}: round-trip OK ({} bytes of YAML)", yaml.len());
        } else {
            failures += 1;
            println!("{id}: ROUND-TRIP LOST CONTENT");
        }
    }
    if failures > 0 {
        return Err(format!("{failures} sheet(s) did not survive the round trip").into());
    }
    Ok(())
}
