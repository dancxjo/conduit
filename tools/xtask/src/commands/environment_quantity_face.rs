//! Pure reported-quantity projection; no installed Source, Mask or authority claim.
use std::{error::Error, path::Path, process::Command};

pub(super) fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    for (package, test) in [
        ("conduit-face", "quantity_observation_contribution"),
        ("conduit-semantic-catalog", "quantity_observation_face"),
    ] {
        super::execute(
            Command::new("cargo")
                .current_dir(root)
                .args(["test", "--locked", "-p", package, "--test", test]),
        )?;
        super::execute(Command::new("cargo").current_dir(root).args([
            "clippy", "--locked", "-p", package, "--lib", "--test", test, "--", "-D", "warnings",
        ]))?;
        super::execute(Command::new("cargo").current_dir(root).args([
            "check",
            "--locked",
            "-p",
            package,
            "--no-default-features",
            "--lib",
            "--target",
            "wasm32-unknown-unknown",
        ]))?;
    }
    Ok(())
}
