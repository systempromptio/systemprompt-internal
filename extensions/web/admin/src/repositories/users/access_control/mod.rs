//! Access-control rule storage and per-user matrix resolution.
//!
//! `rules` owns the CRUD over `access_control_rules`; `matrix` resolves the
//! effective grant for every catalog entity against a single user's rule chain,
//! and `matrix_types` holds the shapes that resolution is rendered into.

mod matrix;
mod matrix_types;
mod rules;

pub use matrix::{filter_catalog_for_user, resolve_user_matrix};
pub use matrix_types::{
    MatrixRow, MatrixSection, MatrixSource, SectionInput, UserMatrix, UserMatrixUser,
};
pub use rules::{
    bulk_set_rules, count_assignments_by_entity_type, list_all_rules, list_rules_for_entity,
    set_entity_rules,
};
