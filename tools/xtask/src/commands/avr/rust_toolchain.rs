//! Exact pinned Rust compiler and source verification, shared with setup dispatch.
use super::require_success;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
pub(super) const RUST_TOOLCHAIN: &str = "nightly-2024-07-22";

pub(super) fn provision_rust() -> Result<(), Box<dyn std::error::Error>> {
    if installed_rust()? {
        return Ok(());
    }
    let output = Command::new("rustup")
        .args([
            "toolchain",
            "install",
            RUST_TOOLCHAIN,
            "--profile",
            "minimal",
            "--component",
            "rust-src",
        ])
        .output()?;
    require_success(&output, "pinned Rust AVR toolchain install")?;
    if !installed_rust()? {
        return Err("pinned Rust AVR installation is incomplete".into());
    }
    Ok(())
}

fn installed_rust() -> Result<bool, Box<dyn std::error::Error>> {
    // `which` only inspects installed toolchains; it does not provision one.
    let output = Command::new("rustup")
        .args(["which", "--toolchain", RUST_TOOLCHAIN, "rustc"])
        .output()?;
    if !output.status.success() {
        return Ok(false);
    }
    let rustc = PathBuf::from(String::from_utf8(output.stdout)?.trim());
    let root = rustc
        .parent()
        .and_then(Path::parent)
        .ok_or("invalid installed rustc path")?;
    let manifest: toml::Value =
        fs::read_to_string(root.join("lib/rustlib/multirust-channel-manifest.toml"))?.parse()?;
    if manifest.get("date").and_then(toml::Value::as_str) != Some("2024-07-22") {
        return Err("cached AVR nightly manifest has the wrong pinned date".into());
    }
    let expected = manifest
        .get("pkg")
        .and_then(|p| p.get("rustc"))
        .and_then(|p| p.get("version"))
        .and_then(toml::Value::as_str)
        .ok_or("AVR nightly rustc identity missing")?;
    let version = Command::new(&rustc).arg("--version").output()?;
    require_success(&version, "verify pinned AVR rustc")?;
    if String::from_utf8(version.stdout)?.trim() != format!("rustc {expected}") {
        return Err("cached AVR nightly compiler does not match its manifest".into());
    }
    let components = fs::read_to_string(root.join("lib/rustlib/components"))?;
    Ok(components.lines().any(|line| line == "rust-src")
        && root
            .join("lib/rustlib/src/rust/library/core/src/lib.rs")
            .is_file()
        && root
            .join("lib/rustlib/src/rust/library/alloc/src/lib.rs")
            .is_file())
}
