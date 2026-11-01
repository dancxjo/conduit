//! Recorded-history proof routes with explicit preparation or installed CPU scope.
use std::{error::Error, path::Path, process::Command};
pub(super) fn dispatch(root: &Path, args: &[String]) -> Option<Result<(), Box<dyn Error>>> {
    match args {
        [name] if name == "quantity-rounding" => Some(quantity_rounding(root)),
        [name] if name == "history" => Some(prove(root)),
        [name] if name == "history-installed-cpu" => Some(installed_cpu(root)),
        [name] if name == "history-native" => Some(native(root)),
        [name] if name == "history-root-startup" => Some(root_startup(root)),
        [name] if name == "history-target" => Some(target(root)),
        [name] if name == "history-browser" => Some(browser(root)),
        [name] if name == "history-preview" => Some(preview(root)),
        _ => None,
    }
}
fn root_startup(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduit-plot",
        "--lib",
        "canonical_expansion_tests::",
    ]))?;
    super::execute(Command::new("cargo").current_dir(root).args([
        "clippy",
        "--locked",
        "-p",
        "conduit-plot",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ]))?;
    super::execute(Command::new("cargo").current_dir(root).args([
        "check",
        "--locked",
        "-p",
        "conduit-plot",
        "--lib",
        "--target",
        "wasm32-unknown-unknown",
    ]))
}
fn command(root: &Path, operation: &str) -> Command {
    let mut command = Command::new("cargo");
    command
        .current_dir(root)
        .args([
            operation,
            "--locked",
            "--manifest-path",
            "bodies/environment/history/Cargo.toml",
        ])
        .env(
            "CARGO_TARGET_DIR",
            std::env::var_os("CARGO_TARGET_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| root.join("target")),
        );
    command
}
fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(&mut command(root, "test"))?;
    super::execute(command(root, "clippy").args(["--all-targets", "--", "-D", "warnings"]))
}
fn target(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(command(root, "check").args(["--lib", "--target", "wasm32-unknown-unknown"]))
}
fn preview(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(
        command(root, "run")
            .args(["--example", "preview", "--"])
            .arg(root.join("target/environment-history-preview")),
    )
}

fn browser(root: &Path) -> Result<(), Box<dyn Error>> {
    preview(root)?;
    let cli = root.join("proof/browser/node_modules/@playwright/test/cli.js");
    if !cli.is_file() {
        super::execute(Command::new("npm").current_dir(root).args([
            "--prefix",
            "proof/browser",
            "ci",
            "--ignore-scripts",
            "--prefer-offline",
        ]))?;
    }
    super::execute(Command::new("node").current_dir(root).arg(cli).args([
        "test",
        "--config",
        "proof/browser/environment-history.config.mjs",
        "--project",
        "chromium",
        "--workers=1",
        "--retries=0",
    ]))
}

// Exact display rounding only: no Look wire, device or live Host migration.
fn quantity_rounding(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduit-face",
        "--test",
        "quantity_rounding",
    ]))?;
    super::execute(Command::new("cargo").current_dir(root).args([
        "clippy",
        "--locked",
        "-p",
        "conduit-face",
        "--lib",
        "--test",
        "quantity_rounding",
        "--",
        "-D",
        "warnings",
    ]))?;
    super::execute(Command::new("cargo").current_dir(root).args([
        "check",
        "--locked",
        "-p",
        "conduit-face",
        "--lib",
        "--target",
        "wasm32-unknown-unknown",
    ]))?;
    for operation in ["test", "clippy"] {
        let mut command = Command::new("cargo");
        command.current_dir(root).args([
            operation,
            "--locked",
            "--manifest-path",
            "plots/seven-segment-mask/Cargo.toml",
        ]);
        if operation == "clippy" {
            command.args(["--all-targets", "--", "-D", "warnings"]);
        }
        super::execute(&mut command)?;
    }
    Ok(())
}

fn native(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "-p",
        "conduitos",
        "--lib",
        "--features",
        "hosted-tools,native-compositor",
        "native_mask_play::history_tests",
    ]))
}

// Actual Std registration/Plan/Play; invented archive, no Body or physical proof.
fn installed_cpu(root: &Path) -> Result<(), Box<dyn Error>> {
    for operation in ["test", "clippy"] {
        let mut command = Command::new("cargo");
        command.current_dir(root).args([
            operation,
            "--locked",
            "--manifest-path",
            "bodies/environment/host/Cargo.toml",
            "--features",
            "history-source",
            "--test",
            "installed_history_source",
        ]);
        if operation == "clippy" {
            command.args(["--lib", "--", "-D", "warnings"]);
        }
        super::execute(&mut command)?;
    }
    Ok(())
}
