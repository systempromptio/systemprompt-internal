//! Data access for the admin surface, one module per domain.
//!
//! Callers path-qualify (`repositories::config::gateway::create_route`);
//! this module re-exports nothing, so the module path is the only name a
//! symbol has and collisions between domains cannot arise.

pub mod access_tokens;
pub mod analytics;
pub mod config;
pub mod dashboard;
pub mod departments;
pub mod evals;
pub mod governance;
pub mod jobs;
pub mod marketplace;
pub mod mcp;
pub mod secrets;
pub mod traces;
pub mod users;
