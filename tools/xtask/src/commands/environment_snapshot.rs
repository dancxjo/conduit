//! Actual Linux snapshot effects under independent selected read bindings.
use std::{error::Error, path::Path, process::Command};
pub(super) fn prove(root: &Path) -> Result<(), Box<dyn Error>> {
    super::execute(Command::new("cargo").current_dir(root).args([
        "test",
        "--locked",
        "--manifest-path",
        "targets/std/resource-snapshot/Cargo.toml",
        "--test",
        "files",
    ]))
}
