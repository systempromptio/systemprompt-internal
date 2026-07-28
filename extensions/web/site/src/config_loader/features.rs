//! Directory-based loading of `services/web/config/features/*.yaml`.

use std::sync::Arc;

use crate::features::{FeaturePage, FeaturePagesConfig};

use super::{ConfigError, load_app_paths};

pub(super) fn load_features_config() -> Result<Option<Arc<FeaturePagesConfig>>, ConfigError> {
    let paths = match load_app_paths() {
        Ok(p) => p,
        Err(e) => {
            tracing::debug!(error = %e, "AppPaths not available for features config");
            return Ok(None);
        },
    };

    let features_dir = paths.system().services().join("web/config/features");

    let Some(entries) = read_features_dir(&features_dir)? else {
        return Ok(None);
    };

    let mut pages = parse_feature_pages(entries)?;

    if pages.is_empty() {
        tracing::debug!("No feature pages loaded");
        return Ok(None);
    }

    pages.sort_by(|a, b| a.slug.cmp(&b.slug));

    tracing::info!(
        page_count = pages.len(),
        "Loaded features config from config/features/"
    );

    Ok(Some(Arc::new(FeaturePagesConfig { pages })))
}
fn read_features_dir(
    features_dir: &std::path::Path,
) -> Result<Option<std::fs::ReadDir>, ConfigError> {
    match std::fs::read_dir(features_dir) {
        Ok(entries) => Ok(Some(entries)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::debug!(
                path = %features_dir.display(),
                "Features config directory does not exist"
            );
            Ok(None)
        },
        Err(e) => Err(ConfigError::Parse {
            config_name: features_dir.display().to_string(),
            message: format!("Failed to read directory: {e}"),
        }),
    }
}
fn parse_feature_pages(entries: std::fs::ReadDir) -> Result<Vec<FeaturePage>, ConfigError> {
    let mut pages: Vec<FeaturePage> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for entry in entries
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "yaml"))
    {
        let path = entry.path();
        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_yaml::from_str(&content) {
                Ok(page) => pages.push(page),
                Err(e) => errors.push(format!("{}: failed to parse: {e}", path.display())),
            },
            Err(e) => errors.push(format!("{}: failed to read: {e}", path.display())),
        }
    }

    if !errors.is_empty() {
        return Err(ConfigError::Parse {
            config_name: "features".to_owned(),
            message: errors.join("; "),
        });
    }

    Ok(pages)
}
