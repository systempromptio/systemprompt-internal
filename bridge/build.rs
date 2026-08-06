//! Build script: embed the Systemprompt icon + metadata into the Windows executable.
//! No-op on non-Windows targets. Mirrors systemprompt-core/bin/bridge/build.rs.
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "cargo build-script protocol uses stdout for `cargo:` directives"
)]
#![allow(
    clippy::expect_used,
    reason = "panicking is the conventional build-script failure mode; a missing brand asset must fail the build"
)]

fn main() {
    // Copy the embedded brand assets into OUT_DIR and declare them as build
    // inputs. `main.rs` `include_bytes!`/`include_str!`s them from OUT_DIR, so
    // regenerating an asset deterministically invalidates the `main.rs`
    // compilation — without this, incremental/sccache builds keep the stale
    // bytes baked into the binary (the window/tray icons go stale while the
    // winresource exe icon, re-read each build, does not).
    // Why: cargo always sets OUT_DIR for a build script, so an absent value means
    // this ran outside cargo and no output path is guessable.
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR set by cargo");
    for f in [
        "window-icon-1024.png",
        "tray-icon.png",
        "icon.svg",
        "logo.svg",
        "theme.css",
    ] {
        let src = format!("assets/{f}");
        // Why: a missing brand asset must stop the build — `main.rs` `include_bytes!`s
        // this path, so continuing would fail later with no reference to the asset.
        std::fs::copy(&src, format!("{out_dir}/{f}")).expect("copy brand asset to OUT_DIR");
        println!("cargo:rerun-if-changed={src}");
    }

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/app-icon.ico");
        res.set("FileDescription", "Systemprompt Internal Bridge");
        res.set("ProductName", "Systemprompt Internal Bridge");
        res.set("CompanyName", "systemprompt.io");
        res.set("LegalCopyright", "Copyright (C) 2026 systemprompt.io.");
        res.set("OriginalFilename", "systemprompt-internal-bridge.exe");
        res.set("InternalName", "systemprompt-internal-bridge");
        if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
            res.set_toolkit_path("/usr/x86_64-w64-mingw32/bin");
            res.set_windres_path("x86_64-w64-mingw32-windres");
            res.set_ar_path("x86_64-w64-mingw32-ar");
        }
        if let Err(e) = res.compile() {
            eprintln!("cargo:warning=winresource compile failed: {e}");
        }
        println!("cargo:rerun-if-changed=assets/app-icon.ico");
        println!("cargo:rerun-if-changed=build.rs");
    }
}
