#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
//! Systemprompt Internal desktop bridge.
//!
//! A thin white-label wrapper over the systemprompt bridge: it defines the
//! Systemprompt [`Brand`] (chrome, on-disk paths, env prefix, default gateway,
//! and embedded GUI assets) and hands it to
//! [`systemprompt_bridge::run_with_brand`]. All behaviour lives in the shared
//! core library — this file is intentionally tiny so a new client bridge is
//! "copy this crate, swap `assets/`, edit the const below". See `README.md` for
//! the recipe.

use std::process::ExitCode;

use systemprompt_bridge::brand::{Brand, BrandAssets};

// Why: the `mod` reference is what keeps this module's `inventory::submit!`s
// linked into the binary — an unreferenced module is dropped before its
// initializers run.
#[cfg(any(target_os = "windows", target_os = "macos"))]
mod registry;

static SYSTEMPROMPT_BRAND: Brand = Brand {
    app_name: "Systemprompt Internal Bridge",
    binary_name: systemprompt_internal_brand::BRIDGE_BINARY_NAME,
    // Why: this crate's version, not the core library's — it is what the
    // `bridge-v*` release tag carries and what the updater compares against the
    // gateway's advertised version.
    version: env!("CARGO_PKG_VERSION"),
    vendor: "systemprompt.io",
    config_dir: "systemprompt-internal",
    config_file: "systemprompt-internal-bridge.toml",
    pat_file: "systemprompt-internal-bridge.pat",
    working_dir_name: "systemprompt-internal-bridge",
    workspace_dir_name: "Systemprompt",
    keyring_service: "systemprompt-internal-bridge.oauth-client",
    env_prefix: "SYSTEMPROMPT_BRIDGE",
    // Why: shipped binaries must point at the production gateway out of the
    // box; local development overrides via SYSTEMPROMPT_BRIDGE_* env or the
    // explicit gateway argument (`just claude <code> http://localhost:8081`).
    default_gateway_url: "https://internal.systemprompt.io",
    // Why: the gateway mounts this page under /bridge-auth (see
    // extensions/web/src/extension_impl.rs), not the upstream default /bridge.
    device_link_path: "/bridge-auth/device-link",
    tray_tooltip: "Systemprompt Bridge",
    window_title: "Systemprompt Bridge",
    app_menu_name: "Systemprompt Bridge",
    sign_in_label: "Sign in with systemprompt",
    sign_in_hint: "Opens your browser — sign in there with your Odoo email and password (or a \
                   passkey). The account you approve as is the one this computer links to.",
    docs_url: "https://systemprompt.io/docs/bridge",
    contact_email: "ed@systemprompt.io",
    pitch_head: "Govern every coding agent.",
    pitch_body: "One gateway. Every agent. Every tool call audited.",
    schedule_label: "io.systemprompt.internal-bridge-sync",
    schedule_unit: "systemprompt-internal-bridge-sync",
    schedule_task_name: "SystempromptBridgeSync",
    // Why: the autostart entry is a *separate* registration from the sync
    // schedule above — it launches the GUI at login, where the schedule runs
    // headless sync — so it needs its own label and task name or the two
    // overwrite each other. `aumid` is the Windows Application User Model ID
    // that groups the taskbar window and owns toast notifications; it must be
    // distinct from the upstream bridge's, or an installed systemprompt bridge
    // and this one collide on the same identity.
    autostart_label: "io.systemprompt.internal-bridge-gui",
    autostart_task_name: "SystempromptInternalBridgeGui",
    aumid: "io.systemprompt.internal-bridge",
    // Why: the systemprompt.io palette is one dark surface with a single orange
    // accent (assets/theme.css) — there is no light theme to switch to, so the
    // GUI and its title bar stay dark whatever the OS asks for.
    force_dark: true,
    // Why: embedded from OUT_DIR (build.rs copies them there) so a regenerated
    // asset re-embeds under incremental/sccache builds.
    assets: BrandAssets {
        icon_svg: include_str!(concat!(env!("OUT_DIR"), "/icon.svg")),
        logo_svg: include_str!(concat!(env!("OUT_DIR"), "/logo.svg")),
        window_icon_png: include_bytes!(concat!(env!("OUT_DIR"), "/window-icon-1024.png")),
        tray_icon_png: include_bytes!(concat!(env!("OUT_DIR"), "/tray-icon.png")),
        app_icon_ico: include_bytes!(concat!(env!("OUT_DIR"), "/app-icon.ico")),
        theme_css: include_str!(concat!(env!("OUT_DIR"), "/theme.css")),
    },
};

fn main() -> ExitCode {
    systemprompt_bridge::run_with_brand(&SYSTEMPROMPT_BRAND)
}
