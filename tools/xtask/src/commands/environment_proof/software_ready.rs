//! Capture actual Root ownership reports, then check Source and compose offers
//! on the planning host. No authority issuance or installation is performed.
use super::{execute, host_target};
use std::{error::Error, path::Path, process::Command};

pub(super) fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    capture_and_plan(root).map(|_| ())
}
pub(super) fn prove_ingress(root: &Path) -> Result<(), Box<dyn Error>> {
    let output = capture_and_plan(root)?;
    execute(
        Command::new("python3")
            .current_dir(root)
            .arg("bodies/environment/tools/native-assignment-ingress.py")
            .arg("--capture")
            .arg(output),
    )
}
pub(super) fn prove_kernel_preparation(root: &Path) -> Result<(), Box<dyn Error>> {
    let output = capture_and_plan(root)?;
    for (script, before) in [
        ("native-kernel-preparation.py", true),
        ("native-assignment-ingress.py", false),
        ("native-kernel-preparation.py", false),
    ] {
        let mut command = Command::new("python3");
        command
            .current_dir(root)
            .arg(root.join("bodies/environment/tools").join(script))
            .arg("--capture")
            .arg(&output);
        if before {
            command.arg("--before-staging");
        }
        execute(&mut command)?;
    }
    Ok(())
}
pub(super) fn prove_body_admission(root: &Path) -> Result<(), Box<dyn Error>> {
    capture_body_admission(root).map(|_| ())
}
pub(super) fn prove_body_construction(root: &Path) -> Result<(), Box<dyn Error>> {
    let admission = capture_body_admission(root)?;
    capture_and_plan_with_body(root, Some(&admission)).map(|_| ())
}
pub(super) fn capture_body_admission(root: &Path) -> Result<std::path::PathBuf, Box<dyn Error>> {
    execute(
        Command::new("cargo")
            .current_dir(root)
            .args([
                "build",
                "--locked",
                "--manifest-path",
                "bodies/environment/make/Cargo.toml",
                "--example",
                "native_admission",
            ])
            .env("CARGO_TARGET_DIR", host_target(root)),
    )?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let output = root.join(format!("target/environment-body-admission/{stamp}"));
    execute(
        Command::new("python3")
            .current_dir(root)
            .arg("bodies/environment/tools/native-body-admission.py")
            .arg("--verifier")
            .arg(host_target(root).join("debug/examples/native_admission"))
            .arg("--output")
            .arg(&output),
    )?;
    Ok(output)
}
fn capture_and_plan(root: &Path) -> Result<std::path::PathBuf, Box<dyn Error>> {
    capture_and_plan_with_body(root, None)
}
pub(super) fn capture_and_plan_with_body(
    root: &Path,
    admission: Option<&Path>,
) -> Result<std::path::PathBuf, Box<dyn Error>> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let output = root.join(format!("target/environment-software-ready/{stamp}"));
    execute(
        Command::new("python3")
            .current_dir(root)
            .arg("bodies/environment/tools/native-software-ready.py")
            .arg("--output")
            .arg(&output),
    )?;
    let mut command = Command::new("cargo");
    command
        .current_dir(root)
        .args([
            "run",
            "--locked",
            "--manifest-path",
            "bodies/environment/make/Cargo.toml",
            "--example",
            "observed_offers",
            "--",
        ])
        .arg(&output)
        .env("CARGO_TARGET_DIR", host_target(root));
    if let Some(admission) = admission {
        command.arg(admission);
    }
    execute(&mut command)?;
    Ok(output)
}
