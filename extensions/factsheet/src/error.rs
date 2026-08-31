//! Errors the factsheet engine can return.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FactsheetError {
    #[error("Factsheet asset not found: {0}")]
    AssetMissing(String),

    #[error(
        "Factsheet '{0}' not found. Call factsheet_list to see the sheets this instance ships."
    )]
    SheetMissing(String),

    #[error("Failed to read {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Invalid sheet definition for '{id}': {message}")]
    Parse { id: String, message: String },

    #[error("Template error: {0}")]
    Template(String),

    /// The renderer subprocess failed. Carries its stderr, because the useful
    /// detail is always `WeasyPrint`'s own message, not the exit code.
    #[error("PDF renderer failed: {0}")]
    Renderer(String),

    /// The two-page budget is a design constraint, not a formatting accident.
    /// Overlong lead prose must fail loudly so the caller shortens and retries
    /// rather than silently shipping a three-page "one-pager".
    #[error(
        "Factsheet '{id}' rendered {pages} pages but its budget is {max}. Shorten the copy — \
         the lede, the capability card bodies and the flow caption are the usual culprits — \
         and render again."
    )]
    PageBudget {
        id: String,
        pages: usize,
        max: usize,
    },
}

pub type FactsheetResult<T> = Result<T, FactsheetError>;
