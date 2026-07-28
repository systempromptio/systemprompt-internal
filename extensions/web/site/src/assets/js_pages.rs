//! Per-page JavaScript bundle definitions.

use std::path::Path;
use systemprompt::extension::AssetDefinition;

macro_rules! page_js {
    ($p:expr, $name:literal) => {
        AssetDefinition::js($p.join($name), concat!("js/pages/", $name))
    };
}

pub(super) fn page_js_assets(storage_js: &Path) -> Vec<AssetDefinition> {
    let pages = storage_js.join("pages");
    vec![
        page_js!(&pages, "admin-access-control.js"),
        page_js!(&pages, "admin-access-control-editors.js"),
        page_js!(&pages, "admin-access-control-matrix.js"),
        page_js!(&pages, "admin-access-control-modals.js"),
        page_js!(&pages, "admin-access-control-state.js"),
        page_js!(&pages, "admin-access-tokens.js"),
        page_js!(&pages, "admin-contexts.js"),
        page_js!(&pages, "admin-demo-register.js"),
        page_js!(&pages, "admin-models.js"),
        page_js!(&pages, "admin-register.js"),
        page_js!(&pages, "admin-register-ui.js"),
        page_js!(&pages, "admin-settings.js"),
        page_js!(&pages, "admin-setup-verified.js"),
        page_js!(&pages, "admin-user-detail.js"),
        page_js!(&pages, "admin-users-actions.js"),
        page_js!(&pages, "admin-users.js"),
        page_js!(&pages, "admin-verify-pending.js"),
        page_js!(&pages, "management-department-detail.js"),
        page_js!(&pages, "management-departments.js"),
    ]
}
