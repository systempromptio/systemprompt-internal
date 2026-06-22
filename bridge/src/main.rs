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
    // WIRE-CONTRACT: this must match the managed-plugin name the Astound gateway
    // emits in its signed manifest. Per the rebrand decision it stays as the
    // shared systemprompt value; only change it alongside a coordinated gateway
    // change.
    synthetic_plugin_name: "systemprompt-managed",
    // TODO(astound): point at the real Astound gateway host before cutting a
    // release. Overridable at runtime via ASTOUND_BRIDGE_GATEWAY_URL or
    // `astound-bridge install --gateway <url>`.
    default_gateway_url: "https://gateway.astounddigital.com",
    // The Astound gateway mounts the device-link consent page under
    // /bridge-auth (see extensions/web/src/extension_impl.rs nest_service),
    // not the upstream default /bridge — keep these in lockstep.
    device_link_path: "/bridge-auth/device-link",
    tray_tooltip: "Astound Bridge",
    window_title: "Astound Bridge",
    app_menu_name: "Astound Bridge",
    assets: BrandAssets {
        icon_svg: include_str!("../assets/icon.svg"),
        logo_svg: include_str!("../assets/logo.svg"),
        window_icon_png: include_bytes!("../assets/window-icon-1024.png"),
        tray_icon_png: include_bytes!("../assets/tray-icon.png"),
        theme_css: include_str!("../assets/theme.css"),
    },
};

fn main() -> ExitCode {
    systemprompt_bridge::run_with_brand(&ASTOUND_BRAND)
}
