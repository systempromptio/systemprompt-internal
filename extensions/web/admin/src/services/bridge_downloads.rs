//! Where the desktop bridge is downloaded from.
//!
//! The bridge ships as GitHub Release `bridge-v<version>` on
//! `systempromptio/systemprompt-internal`, cut by
//! `.github/workflows/release.yml` on every merge to `main`. The version is
//! this crate's own: the workspace, the core pin, and `bridge/Cargo.toml` are
//! held to one number by `scripts/sync-release-version.sh`, so a running
//! gateway links to the bridge built beside it rather than to whatever "latest"
//! happens to be. Asset names are version-less and load-bearing — keep them in
//! lockstep with the release workflow's build matrix and
//! `storage/files/js/pages/admin-bridge-setup.js`.

use systemprompt_internal_brand::BRIDGE_BINARY_NAME;

pub(crate) const BRIDGE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub(crate) const RELEASE_REPO: &str = "systempromptio/systemprompt-internal";

pub(crate) fn release_tag() -> String {
    format!("bridge-v{BRIDGE_VERSION}")
}

pub(crate) fn release_base_url() -> String {
    format!(
        "https://github.com/{RELEASE_REPO}/releases/download/{}",
        release_tag()
    )
}

pub(crate) fn release_page_url() -> String {
    format!(
        "https://github.com/{RELEASE_REPO}/releases/tag/{}",
        release_tag()
    )
}

pub(crate) fn install_command(gateway: &str, code: Option<&str>) -> String {
    let base = release_base_url();
    let code = code.map(|c| format!(" --code {c}")).unwrap_or_default();
    format!(
        "curl -fsSL {base}/install.sh | sh -s -- --download-base {base} --gateway {gateway}{code}"
    )
}

pub(crate) fn asset_name(platform: &str) -> String {
    format!("{BRIDGE_BINARY_NAME}-{platform}")
}
