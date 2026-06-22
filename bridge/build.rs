//! Build script: embed the Astound icon + metadata into the Windows executable.
//! No-op on non-Windows targets. Mirrors systemprompt-core/bin/bridge/build.rs.
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "cargo build-script protocol uses stdout for `cargo:` directives"
)]

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/app-icon.ico");
        res.set("FileDescription", "Astound Bridge");
        res.set("ProductName", "Astound Bridge");
        res.set("CompanyName", "Astound Digital");
        res.set("LegalCopyright", "Copyright (C) 2026 Astound Digital.");
        res.set("OriginalFilename", "astound-bridge.exe");
        res.set("InternalName", "astound-bridge");
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
