//! Reproducible source acquisition and bounded native QEMU tool builds.
use super::{report::sha256_file, ConduitosError};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
const ARCHIVE: &str = "qemu-10.2.1.tar.xz";

pub(super) fn build(
    destination: &Path,
    archive_sha256: &str,
    configure: &[&str],
    patches: &[PathBuf],
    executable: &str,
) -> Result<PathBuf, ConduitosError> {
    fs::create_dir_all(destination).map_err(refusal)?;
    let archive = destination.join(ARCHIVE);
    if !archive.exists() {
        download(
            &format!("https://download.qemu.org/{ARCHIVE}"),
            &archive,
            archive_sha256,
        )?;
    }
    verify(&archive, archive_sha256)?;
    // Failed attempts remain inspectable and never satisfy a warm receipt.
    let attempt = destination.join(format!("attempt-{}", std::process::id()));
    fs::create_dir(&attempt).map_err(refusal)?;
    let source = attempt.join("source");
    let build = attempt.join("build");
    fs::create_dir(&source).map_err(refusal)?;
    fs::create_dir(&build).map_err(refusal)?;
    run(Command::new("tar")
        .args(["--extract", "--file"])
        .arg(&archive)
        .args(["--strip-components=1", "--no-same-owner", "--directory"])
        .arg(&source))?;
    for patch in patches {
        run(Command::new("patch")
            .args(["--batch", "--forward", "--fuzz=0", "-p1", "--input"])
            .arg(patch)
            .current_dir(&source))?;
    }
    run(Command::new(source.join("configure"))
        .args(configure)
        .current_dir(&build))?;
    run(Command::new("ninja")
        .arg("-C")
        .arg(&build)
        .args(["-j", "4", executable]))?;
    Ok(build.join(executable))
}
pub(super) fn verify(path: &Path, expected: &str) -> Result<(), ConduitosError> {
    if sha256_file(path)? != expected {
        return Err(refusal("tool artifact digest mismatch"));
    }
    Ok(())
}
pub(super) fn download(
    url: &str,
    destination: &Path,
    expected: &str,
) -> Result<(), ConduitosError> {
    if destination.exists() {
        return verify(destination, expected);
    }
    let temporary = destination.with_extension(format!("download-{}", std::process::id()));
    run(Command::new("curl")
        .args(["--fail", "--location", "--remove-on-error", "--output"])
        .arg(&temporary)
        .arg(url))?;
    verify(&temporary, expected)?;
    fs::rename(temporary, destination).map_err(refusal)
}
pub(super) fn version(program: &Path, args: &[&str]) -> Result<String, ConduitosError> {
    let output = Command::new(program).args(args).output().map_err(refusal)?;
    if !output.status.success() {
        return Err(refusal(format!(
            "{}: {}",
            program.display(),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned())
}

pub(super) fn run(command: &mut Command) -> Result<(), ConduitosError> {
    let status = command.status().map_err(refusal)?;
    if !status.success() {
        return Err(refusal(format!("{command:?} exited {status}")));
    }
    Ok(())
}
fn refusal(error: impl std::fmt::Display) -> ConduitosError {
    ConduitosError::refusal("qemu-source-preparation-refused", error.to_string())
}
