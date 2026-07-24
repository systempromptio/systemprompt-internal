//! Router construction for the admin plane.

mod admin;
mod ssr;

pub(crate) use admin::{build_admin_only_routes, build_auth_read_routes};
pub use ssr::admin_ssr_router;
