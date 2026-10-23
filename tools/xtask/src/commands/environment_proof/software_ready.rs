//! Capture actual Root ownership reports, then check Source and compose offers
//! on the planning host. No authority issuance or installation is performed.
use super::execute;
use std::{error::Error, path::Path, process::Command};

pub(super) fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
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
            .arg(output)
            .env("CARGO_TARGET_DIR", root.join("target")),
    )
}
