//! The `WebExtension` value and its lazily-loaded configuration handles.
//!
//! Config is parsed once at construction and handed out as `Arc` clones; the
//! trait implementation itself lives in `extension_impl`.

use std::sync::{Arc, OnceLock};

use crate::config::BlogConfigValidated;
use crate::config_loader::{self, ConfigError};
use crate::homepage::HomepageConfig;
use crate::jobs::ContentIngestionJob;
use crate::navigation::NavigationConfig;
use crate::skills_page::SkillsPageConfig;

use systemprompt::extension::prelude::*;

static NAVIGATION_CONFIG: OnceLock<Option<Arc<NavigationConfig>>> = OnceLock::new();
static HOMEPAGE_CONFIG: OnceLock<Option<Arc<HomepageConfig>>> = OnceLock::new();
static SKILLS_PAGE_CONFIG: OnceLock<Option<Arc<SkillsPageConfig>>> = OnceLock::new();
static SALESFORCE_CONFIG: OnceLock<Option<Arc<systemprompt_web_admin::SalesforceConfig>>> =
    OnceLock::new();

#[derive(Debug, Default, Clone)]
pub struct WebExtension {
    pub(crate) validated_config: Option<Arc<BlogConfigValidated>>,
}

impl WebExtension {
    pub const PREFIX: &'static str = "web";

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn with_validated_config(config: Arc<BlogConfigValidated>) -> Self {
        Self {
            validated_config: Some(config),
        }
    }

    #[must_use]
    pub const fn validated_config(&self) -> Option<&Arc<BlogConfigValidated>> {
        self.validated_config.as_ref()
    }

    #[must_use]
    pub const fn base_path() -> &'static str {
        "/api/v1/links"
    }

    #[must_use]
    pub const fn ingestion_job() -> ContentIngestionJob {
        ContentIngestionJob
    }

    #[must_use]
    pub fn navigation_config() -> Option<Arc<NavigationConfig>> {
        log_and_discard_err(
            &NAVIGATION_CONFIG,
            config_loader::load_navigation_config,
            "Navigation config error",
        )
    }

    #[must_use]
    pub fn homepage_config() -> Option<Arc<HomepageConfig>> {
        log_and_discard_err(
            &HOMEPAGE_CONFIG,
            config_loader::load_homepage_config,
            "Homepage config error",
        )
    }

    #[must_use]
    pub fn skills_page_config() -> Option<Arc<SkillsPageConfig>> {
        log_and_discard_err(
            &SKILLS_PAGE_CONFIG,
            config_loader::load_skills_page_config,
            "Skills page config error",
        )
    }

    #[must_use]
    pub fn salesforce_config() -> Option<Arc<systemprompt_web_admin::SalesforceConfig>> {
        log_and_discard_err(
            &SALESFORCE_CONFIG,
            config_loader::load_salesforce_config,
            "Salesforce config error",
        )
    }
}

fn log_and_discard_err<T: Clone>(
    lock: &OnceLock<Option<T>>,
    init: fn() -> Result<Option<T>, ConfigError>,
    msg: &str,
) -> Option<T> {
    lock.get_or_init(|| match init() {
        Ok(val) => val,
        Err(e) => {
            tracing::error!(error = %e, "{msg}");
            None
        },
    })
    .clone()
}

register_extension!(WebExtension);

pub type BlogExtension = WebExtension;
