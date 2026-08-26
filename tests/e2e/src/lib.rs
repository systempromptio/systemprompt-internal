//! End-to-end suite: the FULL API router (gateway + MCP proxy + admin), a
//! throwaway database, and wiremock in place of Odoo.
//!
//! The contract suite mounts only the admin router, so nothing there can
//! reach `/v1/bridge/manifest` or `/api/v1/mcp/<name>/mcp` — the surfaces the
//! bridge and Cowork actually consume. This crate boots the same router the
//! production binary serves, via `AppContextBuilder` + `setup_api_server`,
//! against the real `services/` tree of this checkout, so per-role manifest
//! content, Odoo sign-in with group→role mapping, and MCP proxying are
//! asserted at the wire.
//!
//! Run with nextest (process-per-test): the profile, config, signing
//! authority, and prometheus recorder are all process-global, so each test
//! builds at most one stack.

#[cfg(test)]
mod harness;

#[cfg(test)]
mod health;
#[cfg(test)]
mod manifest_roles;
#[cfg(test)]
mod mcp_proxy_odoo;
#[cfg(test)]
mod odoo_login_roles;
#[cfg(test)]
mod skills_artifacts;

#[cfg(all(test, feature = "live"))]
mod live_smoke;
