//! Build script for rune-cef.
//!
//! This script locates the CEF binary distribution and configures linking.
//! CEF is dynamically linked at runtime, so this mainly sets up paths.

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=CEF_PATH");
    println!("cargo:rerun-if-env-changed=CEF_ROOT");

    // Look for CEF distribution path
    let cef_path = env::var("CEF_PATH")
        .or_else(|_| env::var("CEF_ROOT"))
        .ok()
        .map(PathBuf::from);

    if let Some(ref path) = cef_path {
        // Add library search path for runtime loading
        if cfg!(target_os = "windows") {
            println!("cargo:rustc-link-search=native={}", path.join("Release").display());
        } else if cfg!(target_os = "macos") {
            println!("cargo:rustc-link-search=native={}", path.join("Release").display());
            // Framework path for macOS
            let framework_path = path.join("Release").join("Chromium Embedded Framework.framework");
            if framework_path.exists() {
                println!("cargo:rustc-link-search=framework={}", path.join("Release").display());
            }
        } else {
            println!("cargo:rustc-link-search=native={}", path.join("Release").display());
        }

        // Export CEF path for runtime
        println!("cargo:rustc-env=CEF_LIBRARY_PATH={}", path.join("Release").display());
    }

    // Note: We use dynamic loading via libloading, so we don't link directly to CEF
    // The CEF_LIBRARY_PATH env var is used at runtime to locate the library
}
