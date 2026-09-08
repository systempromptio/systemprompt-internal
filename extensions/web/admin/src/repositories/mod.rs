//! Data access for the admin surface, one module per domain.
//!
//! Callers path-qualify (`repositories::config::gateway::create_route`);
//! this module re-exports nothing, so the module path is the only name a
//! symbol has and collisions between domains cannot arise.

pub mod access_control;
pub mod analytics;
pub mod bridge;
pub mod config;
pub mod dashboard;
pub mod dashboard_read;
pub mod dashboard_reports;
pub mod demo;
pub mod departments;
pub mod dev_login;
pub mod devices;
pub mod external_agents;
pub mod governance;
pub mod groups;
pub mod jobs;
pub mod marketplace;
pub mod mcp;
pub mod organizations;
pub mod overview;
pub mod people_usage;
pub mod projects;
pub mod reports;
pub mod roles;
pub mod scope;
pub mod secrets;
pub mod traces;
pub mod users;
