//! Planned production recovery kernel with actual local Linux file effects.
use super::{execute, host_target};
use std::{error::Error, path::Path, process::Command};
pub(super) fn dispatch(root: &Path, arguments: &[String]) -> Option<Result<(), Box<dyn Error>>> {
    match arguments {
        [command] if command == "recording-files" => Some(prove(root)),
        [command] if command == "catalog-seed" => Some(catalog_seed(root)),
        [command] if command == "catalog-residence" => Some(catalog_residence(root)),
        [command] if command == "catalog-checkpoint" => Some(catalog_checkpoint(root)),
        [command] if command == "catalog-restart" => Some(catalog_restart(root)),
        _ => None,
    }
}
fn catalog_restart(root: &Path) -> Result<(), Box<dyn Error>> {
    if !cfg!(target_os = "linux") {
        return Err("catalog-restart requires Linux".into());
    }
    for operation in ["test", "clippy"] {
        let mut command = Command::new("cargo");
        command
            .current_dir(root)
            .env("CARGO_TARGET_DIR", host_target(root))
            .args([
                operation,
                "--locked",
                "--manifest-path",
                "bodies/environment/make/Cargo.toml",
                "--features",
                "installed-seed-proof",
                "--test",
                "catalog_restart",
            ]);
        if operation == "clippy" {
            command.args(["--", "-D", "warnings"]);
        }
        execute(&mut command)?;
    }
    execute(
        Command::new("cargo")
            .current_dir(root)
            .env("CARGO_TARGET_DIR", host_target(root))
            .args([
                "clippy",
                "--locked",
                "--manifest-path",
                "bodies/environment/linux/Cargo.toml",
                "--features",
                "installed-seed",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ]),
    )
}
fn catalog_checkpoint(root: &Path) -> Result<(), Box<dyn Error>> {
    if !cfg!(target_os = "linux") {
        return Err("catalog-checkpoint requires Linux".into());
    }
    for filter in ["installed_std_tests::catalog_", "state_value::durable::"] {
        execute(
            Command::new("cargo")
                .current_dir(root)
                .env("CARGO_TARGET_DIR", host_target(root))
                .args([
                    "test",
                    "--locked",
                    "-p",
                    "conduit-std-host",
                    "--features",
                    "linux-snapshot",
                    "--lib",
                    filter,
                ]),
        )?;
    }
    execute(
        Command::new("cargo")
            .current_dir(root)
            .env("CARGO_TARGET_DIR", host_target(root))
            .args([
                "clippy",
                "--locked",
                "-p",
                "conduit-std-host",
                "--features",
                "linux-snapshot",
                "--lib",
                "--",
                "-D",
                "warnings",
            ]),
    )
}
pub(super) fn catalog_residence(root: &Path) -> Result<(), Box<dyn Error>> {
    for filter in [
        "installed_std_tests::catalog_seed",
        "state_value::durable::",
    ] {
        execute(
            Command::new("cargo")
                .current_dir(root)
                .env("CARGO_TARGET_DIR", host_target(root))
                .args([
                    "test",
                    "--locked",
                    "-p",
                    "conduit-std-host",
                    "--lib",
                    filter,
                ]),
        )?;
    }
    execute(
        Command::new("cargo")
            .current_dir(root)
            .env("CARGO_TARGET_DIR", host_target(root))
            .args([
                "clippy",
                "--locked",
                "-p",
                "conduit-std-host",
                "--lib",
                "--",
                "-D",
                "warnings",
            ]),
    )
}
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
