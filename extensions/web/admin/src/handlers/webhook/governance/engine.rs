//! The process-wide [`GovernanceEngine`] instance.
//!
//! Core's engine is caller-owned; this module supplies the deployment
//! decisions core leaves open — where the config lives
//! (`<services>/governance/config.yaml` per the profile) and that every
//! enforcement point shares one engine, so the rate limiter sees every call.

use std::path::PathBuf;
use std::sync::LazyLock;

use systemprompt::config::ProfileBootstrap;
use systemprompt_security::policy::{GovernanceConfig, GovernanceEngine};

static ENGINE: LazyLock<GovernanceEngine> = LazyLock::new(|| {
    let config =
        config_path().map_or_else(GovernanceConfig::defaults, |p| GovernanceConfig::load(&p));
    GovernanceEngine::from_config(&config)
});

pub(crate) fn engine() -> &'static GovernanceEngine {
    &ENGINE
}

fn config_path() -> Option<PathBuf> {
    let bootstrap = ProfileBootstrap::get()
        .inspect_err(|e| {
            tracing::error!(
                error = %e,
                "governance profile bootstrap failed; policies fall back to built-in defaults"
            );
        })
        .ok()?;
    Some(PathBuf::from(&bootstrap.paths.services).join("governance/config.yaml"))
}
