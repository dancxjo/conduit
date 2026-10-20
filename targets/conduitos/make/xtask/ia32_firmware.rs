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
pub const REPAIR: &str = "cargo xtask make conduitos prepare-ia32-firmware";
const MANIFEST: &str = include_str!("../../tools/ia32-firmware/manifest.json");
#[derive(Deserialize)]
struct Manifest {
    identity: String,
    package: Package,
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
    bytes: u64,
    sha256: String,
}

fn manifest() -> Result<Manifest> {
    Ok(serde_json::from_str(MANIFEST)?)
}
fn cache(root: &Path) -> PathBuf {
    root.join("target/conduitos/toolchain/ovmf-ia32")
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
/// Exact installed pair is preferred; incomplete/untrusted local cache refuses.
pub fn paths(root: &Path) -> Result<(PathBuf, PathBuf)> {
    resolve(root, Path::new("/usr/share/OVMF"))
        .map_err(|error| format!("unavailable-ia32-firmware: {error}; run `{REPAIR}`").into())
}
fn resolve(root: &Path, installed: &Path) -> Result<(PathBuf, PathBuf)> {
    let pin = manifest()?;
    if let Ok(files) = pair(installed, &pin) {
        return Ok(files);
    }
    pair(&cache(root), &pin)
}
/// Template is read-only; each emulator owns a separate writable variable store.
pub fn copy_vars(template: &Path, output: &Path) -> Result<()> {
    fs::copy(template, output)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(output, fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    {
        let mut permissions = fs::metadata(output)?.permissions();
        permissions.set_readonly(false);
        fs::set_permissions(output, permissions)?;
    }
    Ok(())
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
    prepare_with(root, Path::new("/usr/share/OVMF"), &manifest()?, download)
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
    installed: &Path,
    pin: &Manifest,
    fetch: impl FnOnce(&str, &Path) -> Result<()>,
) -> Result<()> {
    if pair(installed, pin).is_ok() {
        return Ok(());
    }
    let destination = cache(root);
    let parent = destination.parent().ok_or("missing cache parent")?;
    fs::create_dir_all(parent)?;
    // OS lock releases on process death. Keep the inode: unlinking races waiters.
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(parent.join("ovmf-ia32.lock"))?;
    lock.lock()?;
    if destination.exists() {
        pair(&destination, pin)?;
        return Ok(());
    }
    let stage = parent.join(format!("ovmf-ia32-stage-{}", std::process::id()));
    fs::create_dir(&stage)?;
    let result = (|| {
        let package = stage.join("firmware.deb");
        fetch(&pin.package.url, &package)?;
        verify(&package, pin.package.bytes, &pin.package.sha256)?;
        let extracted = stage.join("extracted");
        let status = Command::new("dpkg-deb")
            .arg("--extract")
            .arg(&package)
            .arg(&extracted)
            .status()?;
        if !status.success() {
            return Err(format!("firmware extraction failed: {status}").into());
        }
        let source = extracted.join("usr/share/OVMF");
        pair(&source, pin)?;
        let ready = stage.join("ready");
        fs::create_dir(&ready)?;
        for artifact in &pin.files {
            let output = ready.join(&artifact.name);
            fs::copy(source.join(&artifact.name), &output)?;
            let mut permissions = fs::metadata(&output)?.permissions();
            permissions.set_readonly(true);
            fs::set_permissions(&output, permissions)?;
        }
        fs::write(ready.join("manifest.json"), MANIFEST)?;
        fs::copy(
            extracted.join("usr/share/doc/ovmf-ia32/copyright"),
            ready.join("copyright"),
        )?;
        pair(&ready, pin)?;
        fs::rename(&ready, &destination)?;
        Ok(())
    })();
    fs::remove_dir_all(&stage)?;
    result
}

#[cfg(test)]
#[path = "ia32_firmware_tests.rs"]
mod tests;
