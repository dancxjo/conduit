//! Shared Show custody and environmental HTTP preparation proof.
use super::execute;
use std::{error::Error, path::Path, process::Command};
pub(super) fn dispatch(root: &Path, arguments: &[String]) -> Option<Result<(), Box<dyn Error>>> {
    match arguments {
        [name] if name == "http" => Some(prove(root)),
        [name] if name == "http-target" => Some(target(root)),
        _ => None,
    }
}
fn target(root: &Path) -> Result<(), Box<dyn Error>> {
    execute(
        Command::new("cargo")
            .current_dir(root)
            .args([
                "check",
                "--locked",
                "--manifest-path",
                "bodies/environment/http/Cargo.toml",
                "--lib",
                "--target",
                "thumbv6m-none-eabi",
            ])
            .env(
                "CARGO_TARGET_DIR",
                std::env::var_os("CARGO_TARGET_DIR")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| root.join("target")),
            ),
    )
}
fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    for operation in ["test", "clippy"] {
        let mut shared = Command::new("cargo");
        shared
            .current_dir(root)
            .env("CARGO_TARGET_DIR", &target)
            .args([
                operation,
                "--locked",
                "-p",
                "conduit-face",
                "--no-default-features",
                "--features",
                "show-custody",
                "--test",
                "show_custody",
            ]);
        if operation == "clippy" {
            shared.args(["--", "-D", "warnings"]);
        }
        execute(&mut shared)?;
        let mut consumer = Command::new("cargo");
        consumer
            .current_dir(root)
            .env("CARGO_TARGET_DIR", &target)
            .args([
                operation,
                "--locked",
                "--manifest-path",
                "bodies/environment/http/Cargo.toml",
            ]);
        if operation == "clippy" {
            consumer.args(["--all-targets", "--", "-D", "warnings"]);
        }
        execute(&mut consumer)?;
    }
    execute(
        Command::new("cargo")
            .current_dir(root)
            .env("CARGO_TARGET_DIR", target)
            .args([
                "test",
                "--locked",
                "-p",
                "conduit-face",
                "--no-default-features",
                "--features",
                "show-custody",
                "--doc",
                "show_custody::",
            ]),
    )
}
