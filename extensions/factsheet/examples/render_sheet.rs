//! Render one shipped sheet to PDF, for fidelity checks during the port.
//!
//! `cargo run -p systemprompt-factsheet --example render_sheet -- ceo /tmp/out`

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a developer harness reports to the terminal"
)]

use std::path::PathBuf;
use systemprompt_factsheet::{EnginePaths, FactsheetEngine};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let id = args.next().unwrap_or_else(|| "ceo".to_owned());
    let out_dir = PathBuf::from(args.next().unwrap_or_else(|| "/tmp/factsheet".to_owned()));
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

    println!("sheets: {:?}", engine.list_sheets()?);
    let doc = engine.load_sheet(&id)?;
    let rendered = engine.render_pdf(&doc, &out_dir).await?;
    println!(
        "{} -> {} ({} page(s))",
        rendered.id,
        rendered.pdf_path.display(),
        rendered.page_count
    );
    for image in &rendered.page_images {
        println!("  preview {}", image.display());
    }
    Ok(())
}
