//! The shapes the access matrix is rendered from.
//!
//! Why these are separate from the resolution in [`super::matrix`]: they are
//! the serialisation contract the admin page renders against, and nothing here
//! decides anything. Keeping them apart leaves `matrix.rs` as resolution only,
//! so a change to what a cell *means* cannot be mistaken for a change to how
//! it is drawn.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct UserMatrix {
    pub user: UserMatrixUser,
    pub sections: Vec<MatrixSection>,
}

#[derive(Debug, Serialize)]
pub struct UserMatrixUser {
    pub id: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub roles: Vec<String>,
    pub department: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MatrixSection {
    pub entity_type: String,
    pub label: String,
    pub rows: Vec<MatrixRow>,
}

#[derive(Debug, Serialize)]
pub struct MatrixRow {
    // Why: polymorphic entity reference (gateway_route/mcp_server), no single typed-ID equivalent
    pub entity_id: String,
    pub entity_name: String,
    pub description: Option<String>,
    pub effective: String,
    pub source: MatrixSource,
    pub default_included: bool,
}

#[derive(Debug, Serialize)]
pub struct MatrixSource {
    pub layer: String,
    pub detail: String,
}

/// Section definition supplied by the caller — list of entities of a given
/// kind that exist on this deployment.
pub type SectionInput = (String, String, Vec<(String, String, Option<String>)>);
