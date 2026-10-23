//! Pinned and digest-checked ESP32 flash-tool provisioning shared by launchers.
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
pub(super) const ESPFLASH_VERSION: &str = "4.5.0";
const ESPFLASH_ARCHIVE: &str = "espflash-x86_64-unknown-linux-gnu.zip";
const ESPFLASH_URL: &str = "https://github.com/esp-rs/espflash/releases/download/v4.5.0/espflash-x86_64-unknown-linux-gnu.zip";
pub(super) const ESPFLASH_ARCHIVE_SHA256: &str =
    "dcd7fd822f247df18966935bbb10c7cb4b700c87172349633e9e323ee2542717";
pub(super) const ESPFLASH_BINARY_SHA256: &str =
    "03869e52b4ab6433720087eb8b895a94852e0c6f1e44b07c184b1dfeaab2f6ae";

pub(super) fn provision_espflash(root: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if std::env::consts::OS != "linux" || std::env::consts::ARCH != "x86_64" {
        return Err("pinned espflash bundle currently supports x86_64 Linux only".into());
    }
    let directory = root.join(format!("target/esp32-tools/espflash-{ESPFLASH_VERSION}"));
    let archive = root.join(format!("target/esp32-tools/{ESPFLASH_ARCHIVE}"));
    let binary = directory.join("espflash");
    if binary.is_file() && sha256_file(&binary)? == ESPFLASH_BINARY_SHA256 {
        return Ok(binary);
    }
    fs::create_dir_all(&directory)?;
    let mut download = Command::new("curl");
    download
        .args([
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            ESPFLASH_URL,
            "--output",
        ])
        .arg(&archive);
    require_success(download, "pinned espflash download")?;
    let found = sha256_file(&archive)?;
    if found != ESPFLASH_ARCHIVE_SHA256 {
        return Err(format!(
            "espflash archive digest mismatch: expected {ESPFLASH_ARCHIVE_SHA256}, found {found}"
        )
        .into());
    }
    let mut extract = Command::new("unzip");
    extract.args(["-o"]).arg(&archive).arg("-d").arg(&directory);
    require_success(extract, "pinned espflash extraction")?;
    let found = sha256_file(&binary)?;
    if found != ESPFLASH_BINARY_SHA256 {
        return Err(format!(
            "espflash binary digest mismatch: expected {ESPFLASH_BINARY_SHA256}, found {found}"
        )
        .into());
    }
    Ok(binary)
}

pub(super) fn require_success(
    mut command: Command,
    purpose: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let status = command.status()?;
    if !status.success() {
        return Err(format!("{purpose} failed with status {status}").into());
    }
    Ok(())
}
pub(super) fn sha256_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}
