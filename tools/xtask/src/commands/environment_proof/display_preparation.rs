//! Fresh admitted display Body owner; only dormant preparation is requested.
use super::{execute, host_target};
use std::{
    error::Error,
    path::{Path, PathBuf},
    process::Command,
};
pub(super) fn provision(root: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let keys = super::super::owner_preparation::owner_key_directory()?;
    execute(
        Command::new("cargo")
            .current_dir(root)
            .args([
                "build",
                "--locked",
                "--manifest-path",
                "bodies/environment/make/Cargo.toml",
                "--example",
                "display_owner",
                "--example",
                "native_admission",
                "--example",
                "native_owner_key",
            ])
            .env("CARGO_TARGET_DIR", host_target(root)),
    )?;
    execute(Command::new(host_target(root).join("debug/examples/native_owner_key")).arg(&keys))?;
    Ok(keys)
}
pub(super) fn prepare(
    root: &Path,
    capture: &Path,
    keys: &Path,
    run_idle: bool,
) -> Result<(), Box<dyn Error>> {
    let admission = capture.join("admission");
    execute(
        Command::new("python3")
            .current_dir(root)
            .arg("bodies/environment/tools/native-body-admission.py")
            .arg("--source")
            .arg("display-output")
            .arg("--output")
            .arg(&admission)
            .arg("--verifier")
            .arg(host_target(root).join("debug/examples/native_admission")),
    )?;
    execute(
        Command::new("python3")
            .current_dir(root)
            .arg("bodies/environment/tools/native-owner-preparation.py")
            .arg("--display")
            .arg("--mode")
            .arg(if run_idle { "run" } else { "prepare" })
            .arg("--capture")
            .arg(capture)
            .arg("--admission")
            .arg(admission)
            .arg("--keys")
            .arg(keys)
            .arg("--owner")
            .arg(host_target(root).join("debug/examples/display_owner")),
    )
}
