//! Planned production recovery kernel with actual local Linux file effects.
use super::{execute, host_target};
use std::{error::Error, path::Path, process::Command};
pub(super) fn catalog_seed(root: &Path) -> Result<(), Box<dyn Error>> {
    for (manifest, test) in [
        (
            "architecture/plot/Cargo.toml",
            "retained_resource_reference",
        ),
        ("bodies/environment/make/Cargo.toml", "catalog_seed"),
    ] {
        execute(
            Command::new("cargo")
                .current_dir(root)
                .env("CARGO_TARGET_DIR", host_target(root))
                .args([
                    "test",
                    "--locked",
                    "--manifest-path",
                    manifest,
                    "--test",
                    test,
                ]),
        )?;
        execute(
            Command::new("cargo")
                .current_dir(root)
                .env("CARGO_TARGET_DIR", host_target(root))
                .args([
                    "clippy",
                    "--locked",
                    "--manifest-path",
                    manifest,
                    "--test",
                    test,
                    "--",
                    "-D",
                    "warnings",
                ]),
        )?;
    }
    Ok(())
}
pub(super) fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    if !cfg!(target_os = "linux") {
        return Err("recording-files requires Linux".into());
    }
    execute(
        Command::new("cargo")
            .current_dir(root)
            .env("CARGO_TARGET_DIR", host_target(root))
            .args([
                "test",
                "--locked",
                "--manifest-path",
                "bodies/environment/make/Cargo.toml",
                "--test",
                "recording_recovery",
                "linux_files",
            ]),
    )?;
    execute(
        Command::new("cargo")
            .current_dir(root)
            .env("CARGO_TARGET_DIR", host_target(root))
            .args([
                "clippy",
                "--locked",
                "--manifest-path",
                "bodies/environment/make/Cargo.toml",
                "--test",
                "recording_recovery",
                "--",
                "-D",
                "warnings",
            ]),
    )?;
    execute(
        Command::new("cargo")
            .current_dir(root)
            .env("CARGO_TARGET_DIR", host_target(root))
            .args([
                "clippy",
                "--locked",
                "--manifest-path",
                "bodies/environment/runtime/Cargo.toml",
                "--features",
                "recording-kernel,face-projection",
                "--lib",
                "--",
                "-D",
                "warnings",
            ]),
    )?;
    execute(
        Command::new("cargo")
            .current_dir(root)
            .env("CARGO_TARGET_DIR", host_target(root))
            .args([
                "clippy",
                "--locked",
                "--manifest-path",
                "bodies/environment/linux/Cargo.toml",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ]),
    )
}
