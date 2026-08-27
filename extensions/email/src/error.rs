//! Errors from the SMTP transport.
//!
//! Trimmed relative to the `systemprompt-web` original: that crate serves HTTP
//! and owns tables, so its error type carried `sqlx` and `axum` variants and an
//! `IntoResponse` impl. Nothing here touches a database or a socket the caller
//! did not open, so those variants would be unconstructible.

/// Why an outbound message could not be built or delivered.
#[derive(Debug, thiserror::Error)]
pub enum EmailError {
    // Why: The caller supplied something unsendable — an unparseable address, an
    // empty recipient list, a blank subject or body.
    #[error("Bad request: {0}")]
    BadRequest(String),

    // Why: The message could not be assembled (bad header value, bad body).
    #[error("Failed to build email: {0}")]
    Build(#[from] lettre::error::Error),

    // Why: The relay refused the message, or could not be reached.
    #[error("SMTP transport error: {0}")]
    Smtp(#[from] lettre::transport::smtp::Error),

    // Why: SMTP is not configured for this deployment. Carries the secret keys that
    // were missing, because "email is not configured" is useless on its own to
    // whoever has to fix it.
    #[error(
        "SMTP is not configured: {0}. Set these in the active profile's secrets.json (or as \
         environment variables) and restart the server."
    )]
    NotConfigured(String),
}
