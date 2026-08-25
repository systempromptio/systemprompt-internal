//! Brand constants for Systemprompt Internal.
//!
//! The desktop bridge (`bridge/`, a standalone workspace) and the admin
//! extension both render these; a value that drifted between them produced
//! install instructions naming a binary that does not exist. This leaf crate
//! is the single source both sides import.

// Why: this is the `[[bin]]` name in bridge/Cargo.toml, and what install and
// login commands print; the two must agree or the printed command fails.
pub const BRIDGE_BINARY_NAME: &str = "systemprompt-internal-bridge";
