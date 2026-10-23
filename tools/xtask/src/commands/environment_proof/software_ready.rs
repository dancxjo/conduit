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
pub(super) fn prove_body_admission(root: &Path) -> Result<(), Box<dyn Error>> {
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
    execute(
        Command::new("python3")
            .current_dir(root)
            .arg("bodies/environment/tools/native-body-admission.py")
            .arg("--verifier")
            .arg(host_target(root).join("debug/examples/native_admission"))
            .arg("--output")
            .arg(root.join(format!("target/environment-body-admission/{stamp}"))),
    )
}
fn capture_and_plan(root: &Path) -> Result<std::path::PathBuf, Box<dyn Error>> {
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
    execute(
        Command::new("cargo")
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
            .env("CARGO_TARGET_DIR", host_target(root)),
    )?;
    Ok(output)
}
