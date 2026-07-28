//! Compile-time provider registries for the web sibling crates.
//!
//! Mirrors the governance policy registry pattern: a crate that defines a
//! stateless provider submits it here, next to its definition, and the
//! facade's `Extension` impl collects each family with one call. Adding a
//! renderer or provider is then one file in the crate that owns it, with no
//! edit to the facade.
//!
//! **Boundary:** only zero-argument (stateless) providers register here.
//! Config-parameterized providers (navigation, homepage, per-page feature
//! prerenderers) are constructed explicitly in the facade, where their
//! config is loaded and its absence is handled.
//!
//! Iteration order of `inventory` is unspecified, so every collect sorts by
//! `(priority, source_path)`; `source_path` is set with `file!()` by the
//! submit macros, same as the governance "as code" links.

use std::sync::Arc;

use systemprompt::extension::prelude::{
    ComponentRenderer, ContentDataProvider, PageDataProvider, TemplateDataExtender,
};

#[derive(Debug, Clone, Copy)]
pub struct ComponentRegistration {
    pub build: fn() -> Arc<dyn ComponentRenderer>,
    pub priority: i32,
    pub source_path: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct PageDataRegistration {
    pub build: fn() -> Arc<dyn PageDataProvider>,
    pub priority: i32,
    pub source_path: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct ContentDataRegistration {
    pub build: fn() -> Arc<dyn ContentDataProvider>,
    pub priority: i32,
    pub source_path: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct ExtenderRegistration {
    pub build: fn() -> Arc<dyn TemplateDataExtender>,
    pub priority: i32,
    pub source_path: &'static str,
}

inventory::collect!(ComponentRegistration);
inventory::collect!(PageDataRegistration);
inventory::collect!(ContentDataRegistration);
inventory::collect!(ExtenderRegistration);

macro_rules! collect_sorted {
    ($registration:ty) => {{
        let mut entries: Vec<&$registration> = inventory::iter::<$registration>().collect();
        entries.sort_by_key(|r| (r.priority, r.source_path));
        entries.iter().map(|r| (r.build)()).collect()
    }};
}

#[must_use]
pub fn component_renderers() -> Vec<Arc<dyn ComponentRenderer>> {
    collect_sorted!(ComponentRegistration)
}

#[must_use]
pub fn page_data_providers() -> Vec<Arc<dyn PageDataProvider>> {
    collect_sorted!(PageDataRegistration)
}

#[must_use]
pub fn content_data_providers() -> Vec<Arc<dyn ContentDataProvider>> {
    collect_sorted!(ContentDataRegistration)
}

#[must_use]
pub fn template_data_extenders() -> Vec<Arc<dyn TemplateDataExtender>> {
    collect_sorted!(ExtenderRegistration)
}

#[macro_export]
macro_rules! submit_component {
    ($ctor:expr) => {
        ::inventory::submit!($crate::registry::ComponentRegistration {
            build: || std::sync::Arc::new($ctor),
            priority: 0,
            source_path: file!(),
        });
    };
}

#[macro_export]
macro_rules! submit_page_data {
    ($ctor:expr) => {
        ::inventory::submit!($crate::registry::PageDataRegistration {
            build: || std::sync::Arc::new($ctor),
            priority: 0,
            source_path: file!(),
        });
    };
}

#[macro_export]
macro_rules! submit_content_data {
    ($ctor:expr) => {
        ::inventory::submit!($crate::registry::ContentDataRegistration {
            build: || std::sync::Arc::new($ctor),
            priority: 0,
            source_path: file!(),
        });
    };
}

#[macro_export]
macro_rules! submit_extender {
    ($ctor:expr) => {
        ::inventory::submit!($crate::registry::ExtenderRegistration {
            build: || std::sync::Arc::new($ctor),
            priority: 0,
            source_path: file!(),
        });
    };
}
