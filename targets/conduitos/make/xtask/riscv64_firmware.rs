//! Pinned firmware acquisition is preparation only. Execution never downloads.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub const REPAIR: &str = "cargo xtask make conduitos prepare-riscv64-firmware";
const MANIFEST: &str = include_str!("../../tools/riscv64-firmware/manifest.json");
#[derive(Deserialize)]
struct Manifest {
    identity: String,
    archive: Package,
    files: Vec<Artifact>,
}
#[derive(Deserialize)]
struct Package {
    url: String,
    bytes: u64,
    sha256: String,
}
#[derive(Deserialize)]
struct Artifact {
    name: String,
    member: String,
    bytes: u64,
    sha256: String,
}

fn manifest() -> Result<Manifest> {
    Ok(serde_json::from_str(MANIFEST)?)
}
fn cache(root: &Path) -> PathBuf {
    root.join("target/conduitos/toolchain/edk2-riscv64")
}
fn verify(path: &Path, bytes: u64, hash: &str) -> Result<()> {
    let mut file = File::open(path)?;
    if file.metadata()?.len() != bytes {
        return Err(format!("wrong length: {}", path.display()).into());
    }
    let mut digest = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    if format!("{:x}", digest.finalize()) != hash {
        return Err(format!("wrong SHA-256: {}", path.display()).into());
    }
    Ok(())
}
fn pair(directory: &Path, pin: &Manifest) -> Result<(PathBuf, PathBuf)> {
    for artifact in &pin.files {
        verify(
            &directory.join(&artifact.name),
            artifact.bytes,
            &artifact.sha256,
        )?;
    }
    Ok((
        directory.join(&pin.files[0].name),
        directory.join(&pin.files[1].name),
    ))
}
/// Verify the reviewed repository cache; execution never acquires firmware.
pub fn paths(root: &Path) -> Result<(PathBuf, PathBuf)> {
    pair(&cache(root), &manifest()?)
        .map_err(|error| format!("unavailable-riscv64-firmware: {error}; run `{REPAIR}`").into())
}
pub fn doctor(root: &Path) -> Result<()> {
    let (code, vars) = paths(root)?;
    println!(
        "{}: CODE={} VARS={} (verified immutable template)",
        manifest()?.identity,
        code.display(),
        vars.display()
    );
    Ok(())
}
pub fn prepare(root: &Path) -> Result<()> {
    prepare_with(root, &manifest()?, download)
}
fn download(url: &str, output: &Path) -> Result<()> {
    let status = Command::new("curl")
        .args([
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            "--proto-redir",
            "=https",
            "--proto",
            "=https",
            "--tlsv1.2",
            "--connect-timeout",
            "20",
            "--max-time",
            "180",
            "--output",
        ])
        .arg(output)
        .arg(url)
        .status()?;
    if !status.success() {
        return Err(format!("firmware download failed: {status}").into());
    }
    Ok(())
}
fn prepare_with(
    root: &Path,
    pin: &Manifest,
    fetch: impl FnOnce(&str, &Path) -> Result<()>,
) -> Result<()> {
    let destination = cache(root);
    let parent = destination.parent().ok_or("missing cache parent")?;
    fs::create_dir_all(parent)?;
    // OS lock releases on process death. Keep the inode: unlinking races waiters.
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(parent.join("edk2-riscv64.lock"))?;
    lock.lock()?;
    if destination.exists() {
        pair(&destination, pin)?;
        return Ok(());
    }
    let stage = parent.join(format!("edk2-riscv64-stage-{}", std::process::id()));
    fs::create_dir(&stage)?;
    let result = (|| {
        let archive = stage.join("firmware.tar.xz");
        fetch(&pin.archive.url, &archive)?;
        verify(&archive, pin.archive.bytes, &pin.archive.sha256)?;
        let ready = stage.join("ready");
        fs::create_dir(&ready)?;
        for artifact in &pin.files {
            // Only the reviewed, digest-pinned members are read. Tar never
            // chooses destination paths and never installs files or links.
            let output = ready.join(&artifact.name);
            let file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&output)?;
            let status = Command::new("tar")
                .args(["--extract", "--xz", "--to-stdout", "--file"])
                .arg(&archive)
                .arg("--")
                .arg(&artifact.member)
                .stdout(file)
                .status()?;
            if !status.success() {
                return Err(format!("firmware member extraction failed: {status}").into());
            }
            verify(&output, artifact.bytes, &artifact.sha256)?;
            let mut permissions = fs::metadata(&output)?.permissions();
            permissions.set_readonly(true);
            fs::set_permissions(&output, permissions)?;
        }
        fs::write(ready.join("manifest.json"), MANIFEST)?;
        fs::write(
            ready.join("copyright"),
            include_str!("../../tools/riscv64-firmware/copyright"),
        )?;
        fs::write(
            ready.join("publisher-notice.md"),
            include_str!("../../tools/riscv64-firmware/publisher-notice.md"),
        )?;
        pair(&ready, pin)?;
        fs::rename(&ready, &destination)?;
        Ok(())
    })();
    fs::remove_dir_all(&stage)?;
    result
}

#[cfg(test)]
#[path = "riscv64_firmware_tests.rs"]
mod tests;
