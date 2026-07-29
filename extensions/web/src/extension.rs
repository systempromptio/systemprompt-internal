//! The `WebExtension` value and its lazily-loaded configuration handles.
//!
//! Config loading lives in `systemprompt_web_site::config_loader`, beside
//! the types it deserialises; this type only fronts it for the `Extension`
//! impl in `extension_impl`.

use std::sync::Arc;

use crate::config::BlogConfigValidated;
use crate::SkillsPageConfig;
use crate::homepage::HomepageConfig;
use crate::navigation::NavigationConfig;
use systemprompt_web_site::config_loader;

use systemprompt::extension::prelude::*;

#[derive(Debug, Default, Clone)]
pub struct WebExtension {
    pub(crate) validated_config: Option<Arc<BlogConfigValidated>>,
}

impl WebExtension {
    pub const PREFIX: &'static str = "web";

    #[must_use]
    pub const fn new() -> Self {
        Self {
            validated_config: None,
        }
    }

    /// The blog config shared by the link API and content ingestion.
    ///
    /// Backed by [`BlogConfigValidated::cached`], so every consumer sees the
    /// same load result; a load failure is logged and treated as "no config".
    #[must_use]
    pub fn blog_config() -> Option<Arc<BlogConfigValidated>> {
        match BlogConfigValidated::cached() {
            Ok(config) => config,
            Err(message) => {
                tracing::error!(
                    error = %message,
                    "Blog config error: link generation and content APIs will run unconfigured"
                );
                None
            },
        }
    }

    #[must_use]
    pub fn navigation_config() -> Option<Arc<NavigationConfig>> {
        config_loader::navigation_config()
    }

    #[must_use]
    pub fn homepage_config() -> Option<Arc<HomepageConfig>> {
        config_loader::homepage_config()
    }

    #[must_use]
    pub fn skills_page_config() -> Option<Arc<SkillsPageConfig>> {
        config_loader::skills_page_config()
    }

    #[must_use]
    pub fn salesforce_config() -> Option<Arc<systemprompt_web_admin::SalesforceConfig>> {
        config_loader::salesforce_config()
    }

}

register_extension!(WebExtension);
