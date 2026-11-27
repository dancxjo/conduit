//! Owned State/content preparation receipts needed by the ordinary Body recorder.
use std::{error::Error, io::Write, path::Path, process::Command};

pub(super) fn layout(root: &Path) -> Result<(), Box<dyn Error>> {
    for (package, target, minimum) in [
        ("conduit-core", "sealed_state", 7),
        ("conduit-core", "state_initialization_layout", 3),
        ("conduit-core", "canonical_value_storage", 2),
        ("conduit-core", "structured_info", 1),
        ("conduit-core", "selected_fore_verification_layout", 3),
        ("conduit-plan-lowering", "owned_state_content_layout", 2),
        ("conduit-plan-lowering", "bounded_local_layout", 4),
        ("conduit-plan-lowering", "owned_fore_line_layout", 3),
    ] {
        tests(root, package, target, minimum, None)?;
    }

    super::execute(Command::new("cargo").current_dir(root).args([
        "clippy",
        "--locked",
        "-p",
        "conduit-core",
        "-p",
        "conduit-plan-lowering",
        "--lib",
        "--tests",
        "--",
        "-D",
        "warnings",
    ]))?;
    super::execute(Command::new("cargo").current_dir(root).args([
        "check",
        "--locked",
        "-p",
        "conduit-core",
        "--no-default-features",
        "--target",
        "thumbv6m-none-eabi",
    ]))
}

/// Decode/preparation qualification of the ordinary optional reference constructor.
pub(super) fn expression(root: &Path) -> Result<(), Box<dyn Error>> {
    tests(root, "conduit-plot", "prepared_expression_storage", 4, None)?;
    tests(
        root,
        "conduit-semantic-catalog",
        "prepared_expression_storage",
        1,
        Some("kernel-step"),
    )?;
    tests(
        root,
        "conduit-semantic-catalog",
        "pure_expression_contract_storage",
        3,
        Some("kernel-step"),
    )?;
    super::execute(Command::new("cargo").current_dir(root).args([
        "clippy",
        "--locked",
        "-p",
        "conduit-plot",
        "-p",
        "conduit-semantic-catalog",
        "--features",
        "conduit-semantic-catalog/kernel-step",
        "--lib",
        "--tests",
        "--",
        "-D",
        "warnings",
    ]))
}
fn tests(
    root: &Path,
    package: &str,
    target: &str,
    minimum: usize,
    features: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let mut command = Command::new("cargo");
    command
        .current_dir(root)
        .args(["test", "--locked", "-p", package, "--test", target]);
    if let Some(features) = features {
        command.args(["--features", features]);
    }
    let result = command.output()?;
    std::io::stdout().write_all(&result.stdout)?;
    std::io::stderr().write_all(&result.stderr)?;
    let qualified = String::from_utf8_lossy(&result.stdout).lines().any(|line| {
        line.strip_prefix("test result: ok. ")
            .and_then(|line| line.split_once(" passed;"))
            .is_some_and(|(count, rest)| {
                count.parse::<usize>().is_ok_and(|n| n >= minimum)
                    && rest.starts_with(" 0 failed; 0 ignored;")
            })
    });
    if !result.status.success() || !qualified {
        return Err(format!("Body residence owned-layout proof failed: {package}/{target}").into());
    }
    Ok(())
}
