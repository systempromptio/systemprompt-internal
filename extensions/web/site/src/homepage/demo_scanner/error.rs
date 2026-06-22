use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DemoScanError {
    #[error("demo root not found: {0}")]
    RootNotFound(PathBuf),
    #[error("read {path}: {source}")]
    ReadManifest {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("parse {path}: {source}")]
    ParseManifest {
        path: PathBuf,
        source: serde_yaml::Error,
    },
}
