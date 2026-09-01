//! Persistence for users: identity, access, activity, devices, and usage.

pub mod access_control;
pub mod activity;
pub mod devices;
pub mod federated;
pub mod mutations;
pub mod odoo_identity;
pub mod passkey;
pub mod queries;
pub mod share_token;
pub mod user_queries;
pub mod user_settings;

pub use mutations::{create_user, delete_user, update_user};
pub use share_token::find_share_token_version;
