//! Finite exact display output possession and existing Mask/packet regression.
use super::{execute, host_target};
#[path = "display_preparation.rs"]
mod display_preparation;
#[path = "display_readiness.rs"]
mod display_readiness;
use std::{error::Error, path::Path, process::Command};
pub(super) fn prove(root: &Path, arguments: &[String]) -> Result<(), Box<dyn Error>> {
    if arguments == ["esp32-readiness"]
        || arguments == ["esp32-readiness", "--flash"]
        || arguments == ["esp32-readiness", "--flash", "--stage"]
        || arguments == ["esp32-readiness", "--flash", "--prepare"]
        || arguments == ["esp32-readiness", "--flash", "--run-idle"]
    {
        let run_idle = arguments.last().is_some_and(|v| v == "--run-idle");
        let preparing = run_idle || arguments.last().is_some_and(|v| v == "--prepare");
        let keys = if preparing {
            Some(display_preparation::provision(root)?)
        } else {
            None
        };
        build_readiness(root, keys.as_deref())?;
        return if arguments.len() >= 2 {
            display_readiness::flash_and_capture(
                root,
                arguments.len() == 3,
                keys.as_deref(),
                run_idle,
            )
        } else {
            Ok(())
        };
    }
    if !arguments.is_empty() {
        return Err(
            "expected display-output [esp32-readiness [--flash [--stage|--prepare|--run-idle]]]"
                .into(),
        );
    }
    for operation in ["test", "clippy"] {
        let mut command = Command::new("cargo");
        command
            .current_dir(root)
            .env("CARGO_TARGET_DIR", host_target(root))
            .args([
                operation,
                "--locked",
                "-p",
                "conduit-assigned-plan",
                "-p",
                "conduit-embedded-build",
            ]);
        if operation == "clippy" {
            command.args(["--all-targets", "--", "-D", "warnings"]);
        }
        execute(&mut command)?;
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
                "mechanisms/io/tm1637/Cargo.toml",
                "--features",
                "host-call",
            ]);
        if operation == "clippy" {
            command.args(["--all-targets", "--", "-D", "warnings"]);
        }
        execute(&mut command)?;
    }
    for (manifest, test) in [
        ("architecture/plot/Cargo.toml", "prepared_literal_children"),
        ("bodies/environment/runtime/Cargo.toml", "display_packets"),
        (
            "bodies/environment/runtime/Cargo.toml",
            "display_frame_material",
        ),
        ("bodies/environment/make/Cargo.toml", "native_face_mask"),
        ("bodies/environment/make/Cargo.toml", "display_output"),
        ("bodies/environment/make/Cargo.toml", "display_mask_source"),
    ] {
        for operation in ["test", "clippy"] {
            let mut command = Command::new("cargo");
            command
                .current_dir(root)
                .env("CARGO_TARGET_DIR", host_target(root))
                .args([
                    operation,
                    "--locked",
                    "--manifest-path",
                    manifest,
                    "--test",
                    test,
                ]);
            if operation == "clippy" {
                command.args(["--", "-D", "warnings"]);
            }
            execute(&mut command)?;
        }
    }
    execute(
        Command::new("cargo")
            .current_dir(root)
            .env("CARGO_TARGET_DIR", host_target(root))
            .args([
                "clippy",
                "--locked",
                "--manifest-path",
                "bodies/environment/make/Cargo.toml",
                "--example",
                "display_offers",
                "--",
                "-D",
                "warnings",
            ]),
    )?;
    Ok(())
}

fn build_readiness(root: &Path, keys: Option<&Path>) -> Result<(), Box<dyn Error>> {
    let package = root.join("targets/esp32/proof/tm1637");
    let toolchain: toml::Value = toml::from_str(&std::fs::read_to_string(
        package.join("rust-toolchain.toml"),
    )?)?;
    let channel = toolchain
        .get("toolchain")
        .and_then(|t| t.get("channel"))
        .and_then(toml::Value::as_str)
        .ok_or("missing ESP32 toolchain")?;
    let mut command = Command::new("cargo");
    command
        .current_dir(package)
        .arg(format!("+{channel}"))
        .args([
            "build",
            "--locked",
            "--release",
            "--features",
            "display-ready-root",
        ])
        .env("CARGO_TARGET_DIR", root.join("target/environment-esp32"));
    if let Some(keys) = keys {
        command.env(
            "CONDUIT_ENVIRONMENT_OWNER_PUBLIC_KEY_FILE",
            keys.join("owner.pub"),
        );
    }
    execute(&mut command)
}
