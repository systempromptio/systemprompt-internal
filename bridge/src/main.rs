#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
//! Astound Digital desktop bridge.
//!
//! A thin white-label wrapper over the systemprompt bridge: it defines the
//! Astound [`Brand`] (chrome, on-disk paths, env prefix, default gateway, and
//! embedded GUI assets) and hands it to [`systemprompt_bridge::run_with_brand`].
//! All behaviour lives in the shared core library — this file is intentionally
//! tiny so a new client bridge is "copy this crate, swap `assets/`, edit the
//! const below". See `README.md` for the recipe.

use std::process::ExitCode;

use systemprompt_bridge::brand::{Brand, BrandAssets};

// Astound behaviour registered through core's `inventory` seams. The GUI
// (hence the marketplace-source seam) is win/mac-only, so this module compiles
// there; the `mod` reference keeps its `inventory::submit!`s linked into the
// binary (an unreferenced module would be dropped before its initializers run).
#[cfg(any(target_os = "windows", target_os = "macos"))]
mod registry;

static ASTOUND_BRAND: Brand = Brand {
    app_name: "Astound Bridge",
    binary_name: "astound-bridge",
    vendor: "Astound Digital",
    config_dir: "astound",
    config_file: "astound-bridge.toml",
    pat_file: "astound-bridge.pat",
    working_dir_name: "astound-bridge",
    keyring_service: "astound-bridge.oauth-client",
    env_prefix: "ASTOUND_BRIDGE",
    // Pre-fills the setup/settings gateway field with the local gateway so a dev
    // build talks to a `just start` server out of the box. Point at the deployed
    // Astound gateway host before cutting a release. Overridable at runtime via
    // ASTOUND_BRIDGE_GATEWAY_URL or `astound-bridge install --gateway <url>`.
    default_gateway_url: "http://localhost:8080",
    // The Astound gateway mounts the device-link consent page under
    // /bridge-auth (see extensions/web/src/extension_impl.rs nest_service),
    // not the upstream default /bridge — keep these in lockstep.
    device_link_path: "/bridge-auth/device-link",
    tray_tooltip: "Astound Bridge",
    window_title: "Astound Bridge",
    app_menu_name: "Astound Bridge",
    // The Astound gateway federates identity through Salesforce, so the one-click
    // setup button drives the device-link flow into the gateway's "Sign in with
    // Salesforce" login. The bridge never speaks to Salesforce directly — it only
    // carries the gateway credential the device-link approval returns.
    sign_in_label: "Sign in with Salesforce",
    sign_in_hint: "Opens your browser to sign in with Salesforce on the Astound gateway; this device is linked automatically.",
    // Embedded from OUT_DIR (copied there by build.rs) rather than directly from
    // `assets/`, so regenerating an asset reliably re-embeds it even under
    // incremental/sccache builds. See build.rs.
    assets: BrandAssets {
        icon_svg: include_str!(concat!(env!("OUT_DIR"), "/icon.svg")),
        logo_svg: include_str!(concat!(env!("OUT_DIR"), "/logo.svg")),
        window_icon_png: include_bytes!(concat!(env!("OUT_DIR"), "/window-icon-1024.png")),
        tray_icon_png: include_bytes!(concat!(env!("OUT_DIR"), "/tray-icon.png")),
        theme_css: include_str!(concat!(env!("OUT_DIR"), "/theme.css")),
    },
};

fn main() -> ExitCode {
    systemprompt_bridge::run_with_brand(&ASTOUND_BRAND)
}
