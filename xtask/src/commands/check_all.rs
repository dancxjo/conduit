use std::{path::Path, process::Command};

fn run_step(label: &str, command: &[&str], cwd: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Running check-all step: {label} ===");
    let status = Command::new(command[0])
        .args(&command[1..])
        .current_dir(cwd)
        .status()?;

    if !status.success() {
        return Err(format!("Step '{label}' failed with status {status}").into());
    }
    Ok(())
}

pub fn run(workspace_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting canonical workspace check-all pipeline...");

    run_step(
        "cargo fmt --all --check",
        &["cargo", "fmt", "--all", "--check"],
        workspace_root,
    )?;

    run_step(
        "cargo clippy",
        &[
            "cargo",
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
        workspace_root,
    )?;

    run_step(
        "cargo test --workspace",
        &["cargo", "test", "--workspace"],
        workspace_root,
    )?;

    run_step(
        "cargo check conduit-core thumbv6m-none-eabi",
        &[
            "cargo",
            "check",
            "-p",
            "conduit-core",
            "--no-default-features",
            "--target",
            "thumbv6m-none-eabi",
        ],
        workspace_root,
    )?;

    run_step(
        "cargo check conduit-embedded thumbv6m-none-eabi",
        &[
            "cargo",
            "check",
            "-p",
            "conduit-embedded",
            "--target",
            "thumbv6m-none-eabi",
        ],
        workspace_root,
    )?;

    run_step(
        "conduct assets check",
        &[
            "cargo",
            "run",
            "-p",
            "conduct",
            "--bin",
            "generate-conduct-assets",
            "--",
            "--check",
        ],
        workspace_root,
    )?;

    run_step(
        "conduct example check",
        &[
            "cargo",
            "run",
            "-p",
            "conduct",
            "--",
            "--check",
            "examples/hello.panel",
        ],
        workspace_root,
    )?;

    println!("\n=== Running check-all step: xtask generate-browser-plan --check ===");
    crate::commands::generate_browser_plan::run(workspace_root, true)?;

    println!("\n=== Running check-all step: xtask verify-canonical ===");
    crate::commands::verify_canonical::run(workspace_root, None, false)?;

    println!("\n=== Running check-all step: xtask check-python-boundary ===");
    crate::commands::check_python_boundary::run(workspace_root)?;

    println!("\n=== Running check-all step: xtask performance-gate ===");
    crate::commands::performance_gate::run(workspace_root, false)?;

    println!("\n=== Running check-all step: xtask embedded-gate ===");
    crate::commands::embedded_gate::run(workspace_root)?;

    println!("\n=== Running check-all step: xtask adversarial-profile ===");
    crate::commands::adversarial_profile::run(workspace_root, "constrained")?;

    println!("\nSUCCESS: All workspace check-all steps passed!");
    Ok(())
}
