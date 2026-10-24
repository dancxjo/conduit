//! Finite exact display output possession and existing Mask/packet regression.
use super::{execute, host_target};
#[path = "display_readiness.rs"]
mod display_readiness;
use std::{error::Error, path::Path, process::Command};
pub(super) fn prove(root: &Path, arguments: &[String]) -> Result<(), Box<dyn Error>> {
    if arguments == ["esp32-readiness"] || arguments == ["esp32-readiness", "--flash"] {
        build_readiness(root)?;
        return if arguments.len() == 2 {
            display_readiness::flash_and_capture(root)
        } else {
            Ok(())
        };
    }
    if !arguments.is_empty() {
        return Err("expected display-output [esp32-readiness]".into());
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
        ("bodies/environment/runtime/Cargo.toml", "display_packets"),
        ("bodies/environment/make/Cargo.toml", "native_face_mask"),
        ("bodies/environment/make/Cargo.toml", "display_output"),
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

fn build_readiness(root: &Path) -> Result<(), Box<dyn Error>> {
    let package = root.join("targets/esp32/proof/tm1637");
    let toolchain: toml::Value = toml::from_str(&std::fs::read_to_string(
        package.join("rust-toolchain.toml"),
    )?)?;
    let channel = toolchain
        .get("toolchain")
        .and_then(|t| t.get("channel"))
        .and_then(toml::Value::as_str)
        .ok_or("missing ESP32 toolchain")?;
    execute(
        Command::new("cargo")
            .current_dir(package)
            .arg(format!("+{channel}"))
            .args([
                "build",
                "--locked",
                "--release",
                "--features",
                "display-ready-root",
            ])
            .env("CARGO_TARGET_DIR", root.join("target/environment-esp32")),
    )
}
