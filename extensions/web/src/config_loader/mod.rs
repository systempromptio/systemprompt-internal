//! Bootstrap-time loading of the `services/web/` YAML tree.
//!
//! Runs once at extension construction, before any request is served, so the
//! file-system reads here are not on a hot path.

use std::sync::Arc;

use systemprompt::config::ProfileBootstrap;
use systemprompt::models::AppPaths;
use thiserror::Error;

mod skills;

pub(crate) use skills::load_skills_page_config;

fn load_app_paths() -> Result<AppPaths, ConfigError> {
    let profile =
        ProfileBootstrap::get().map_err(|e| ConfigError::PathsUnavailable(e.to_string()))?;
    AppPaths::from_profile(&profile.paths).map_err(|e| ConfigError::PathsUnavailable(e.to_string()))
}

use crate::homepage::HomepageConfig;
use crate::navigation::{BrandingConfig, NavigationConfig};

#[derive(Debug, Clone, Error)]
pub(crate) enum ConfigError {
    #[error("Failed to parse {config_name}: {message}")]
    Parse {
        config_name: String,
        message: String,
    },

    #[error("Application paths unavailable: {0}")]
    PathsUnavailable(String),
}

pub(crate) fn load_navigation_config() -> Result<Option<Arc<NavigationConfig>>, ConfigError> {
    let Some(nav_value) = load_config_section("navigation.yaml")? else {
        return Ok(None);
    };

    let nav_config: NavigationConfig =
        serde_yaml::from_value(nav_value).map_err(|e| ConfigError::Parse {
            config_name: "navigation.yaml".to_owned(),
            message: e.to_string(),
        })?;

    tracing::info!("Loaded navigation config from config/navigation.yaml");

    Ok(Some(Arc::new(nav_config)))
}

pub(crate) fn load_homepage_config() -> Result<Option<Arc<HomepageConfig>>, ConfigError> {
    let Some(homepage_value) = load_config_section("homepage.yaml")? else {
        return Ok(None);
    };

    let mut homepage_config: HomepageConfig =
        serde_yaml::from_value(homepage_value).map_err(|e| ConfigError::Parse {
            config_name: "homepage.yaml".to_owned(),
            message: e.to_string(),
        })?;

    if let Ok(paths) = load_app_paths() {
        populate_demo_showcase(
            &mut homepage_config,
            paths.system().root().join("demo").as_path(),
        );
    }

    tracing::info!("Loaded homepage config from config/homepage.yaml");

    Ok(Some(Arc::new(homepage_config)))
}

fn populate_demo_showcase(homepage_config: &mut HomepageConfig, demo_root: &std::path::Path) {
    match crate::homepage::demo_scanner::scan_demos(demo_root) {
        Ok(mut scanned) => {
            if let Some(existing) = homepage_config.demos.as_ref() {
                if existing.title.is_some() {
                    scanned.title.clone_from(&existing.title);
                }
                if existing.subtitle.is_some() {
                    scanned.subtitle.clone_from(&existing.subtitle);
                }
            }
            let total_categories: usize = scanned.pillars.iter().map(|p| p.categories.len()).sum();
            tracing::info!(
                pillars = scanned.pillars.len(),
                categories = total_categories,
                "Scanned demo/ for homepage showcase"
            );
            homepage_config.demos = Some(scanned);
        },
        Err(e) => {
            tracing::warn!(
                error = %e,
                path = %demo_root.display(),
                "Failed to scan demo/ directory — homepage will render without demo cards"
            );
        },
    }
}

pub(crate) fn load_salesforce_config()
-> Result<Option<Arc<systemprompt_web_admin::SalesforceConfig>>, ConfigError> {
    let Some(value) = load_config_section("salesforce.yaml")? else {
        return Ok(None);
    };

    let config: systemprompt_web_admin::SalesforceConfig =
        serde_yaml::from_value(value).map_err(|e| ConfigError::Parse {
            config_name: "salesforce.yaml".to_owned(),
            message: e.to_string(),
        })?;

    tracing::info!(
        enabled = config.enabled,
        "Loaded Salesforce SSO config from config/salesforce.yaml"
    );

    Ok(Some(Arc::new(config)))
}

pub(crate) fn load_branding_config() -> Result<Option<BrandingConfig>, ConfigError> {
    let Some(theme_value) = load_config_section("theme.yaml")? else {
        return Ok(None);
    };

    let Some(branding_value) = theme_value.get("branding") else {
        return Ok(None);
    };

    let branding_config: BrandingConfig =
        serde_yaml::from_value(branding_value.clone()).map_err(|e| ConfigError::Parse {
            config_name: "theme.yaml (branding section)".to_owned(),
            message: e.to_string(),
        })?;

    tracing::info!("Loaded branding config from config/theme.yaml");

    Ok(Some(branding_config))
}

/// Branding for callers that render with or without it.
///
/// Branding is optional everywhere it is consumed, so a load failure degrades
/// to the unbranded default rather than failing the page. Both call sites go
/// through here so the failure is reported once, at one level.
pub(crate) fn branding_config() -> Option<BrandingConfig> {
    load_branding_config().unwrap_or_else(|e| {
        tracing::warn!(error = %e, "Failed to load branding config");
        None
    })
}

fn load_config_section(filename: &str) -> Result<Option<serde_yaml::Value>, ConfigError> {
    let paths = match load_app_paths() {
        Ok(p) => p,
        Err(e) => {
            tracing::debug!(error = %e, "AppPaths not available for config section");
            return Ok(None);
        },
    };

    let config_path = paths
        .system()
        .services()
        .join(format!("web/config/{filename}"));

    let yaml_content = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::debug!(
                path = %config_path.display(),
                "Config file does not exist"
            );
            return Ok(None);
        },
        Err(e) => {
            return Err(ConfigError::Parse {
                config_name: filename.to_owned(),
                message: format!("Failed to read file: {e}"),
            });
        },
    };

    serde_yaml::from_str(&yaml_content)
        .map(Some)
        .map_err(|e| ConfigError::Parse {
            config_name: filename.to_owned(),
            message: e.to_string(),
        })
}
