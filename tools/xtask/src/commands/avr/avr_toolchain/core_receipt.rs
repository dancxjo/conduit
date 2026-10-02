//! Reuse only a complete pinned package tree recorded after successful installation.
use super::{
    avr_gcc_bin, config_path, require_success, sha256_file, ARDUINO_AVR_VERSION,
    SPARKFUN_AVR_VERSION,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path, process::Command};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const RECEIPT: &str = "target/avr-promicro/core-verification.json";
const PACKAGES: &str = "target/avr-promicro/arduino/data/packages";
const MAX_FILES: usize = 100_000;
const MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema: String,
    arduino: String,
    sparkfun: String,
    files: BTreeMap<String, Entry>,
}
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Entry {
    sha256: String,
    bytes: u64,
    executable: bool,
    symlink: Option<String>,
}
pub(super) fn verify(root: &Path) -> Result<bool> {
    let file = root.join(RECEIPT);
    if !file.try_exists()? {
        return Ok(false);
    }
    if fs::symlink_metadata(&file)?.file_type().is_symlink()
        || fs::metadata(&file)?.len() > 32 * 1024 * 1024
    {
        return Err("invalid AVR core verification receipt".into());
    }
    let expected: Receipt = serde_json::from_slice(&fs::read(file)?)?;
    if expected != snapshot(root)? {
        return Err("cached AVR core/tool content changed; refusing verified reuse".into());
    }
    check_gcc(root)?;
    Ok(true)
}
pub(super) fn seal(cli: &Path, root: &Path) -> Result<()> {
    let output = Command::new(cli)
        .args(["core", "list", "--format", "json", "--config-file"])
        .arg(config_path(root))
        .output()?;
    require_success(&output, "verify installed pinned AVR cores")?;
    verify_versions(&serde_json::from_slice(&output.stdout)?)?;
    check_gcc(root)?;
    let receipt = snapshot(root)?;
    let temporary = root.join(format!("{RECEIPT}.tmp"));
    fs::write(&temporary, serde_json::to_vec_pretty(&receipt)?)?;
    fs::rename(temporary, root.join(RECEIPT))?;
    Ok(())
}
fn verify_versions(list: &serde_json::Value) -> Result<()> {
    let platforms = list
        .get("platforms")
        .and_then(serde_json::Value::as_array)
        .ok_or("Arduino CLI omitted installed platform list")?;
    for (id, version) in [
        ("arduino:avr", ARDUINO_AVR_VERSION),
        ("SparkFun:avr", SPARKFUN_AVR_VERSION),
    ] {
        let matches = platforms
            .iter()
            .filter(|platform| platform["id"] == id)
            .collect::<Vec<_>>();
        if matches.len() != 1 || matches[0]["installed_version"] != version {
            return Err(format!("installed {id} is not pinned version {version}").into());
        }
    }
    Ok(())
}
fn check_gcc(root: &Path) -> Result<()> {
    let output = Command::new(avr_gcc_bin(root).join("avr-gcc"))
        .arg("-dumpfullversion")
        .output()?;
    require_success(&output, "verify installed pinned AVR GCC")?;
    if String::from_utf8(output.stdout)?.trim() != "7.3.0" {
        return Err("installed AVR GCC is not pinned version 7.3.0".into());
    }
    Ok(())
}
fn snapshot(root: &Path) -> Result<Receipt> {
    let packages = root.join(PACKAGES);
    if fs::symlink_metadata(&packages)?.file_type().is_symlink() {
        return Err("AVR package root cannot be a symlink".into());
    }
    for (vendor, version) in [
        ("arduino", ARDUINO_AVR_VERSION),
        ("SparkFun", SPARKFUN_AVR_VERSION),
    ] {
        let platform = packages
            .join(vendor)
            .join("hardware/avr")
            .join(version)
            .join("platform.txt");
        if !platform.is_file() {
            return Err(format!("installed {vendor} AVR core {version} is absent").into());
        }
    }
    let mut files = BTreeMap::new();
    let mut total = 0;
    inventory(&packages, &packages, &mut files, &mut total, 0)?;
    if files.is_empty() {
        return Err("empty AVR core/tool installation".into());
    }
    Ok(Receipt {
        schema: "conduit.avr-core-installation/v1".into(),
        arduino: ARDUINO_AVR_VERSION.into(),
        sparkfun: SPARKFUN_AVR_VERSION.into(),
        files,
    })
}
fn inventory(
    root: &Path,
    directory: &Path,
    files: &mut BTreeMap<String, Entry>,
    total: &mut u64,
    depth: usize,
) -> Result<()> {
    if depth > 32 {
        return Err("AVR package depth exceeds bound".into());
    }
    for child in fs::read_dir(directory)? {
        let path = child?.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.is_dir() {
            inventory(root, &path, files, total, depth + 1)?;
            continue;
        }
        let link = if metadata.file_type().is_symlink() {
            let canonical = fs::canonicalize(&path)?;
            if !canonical.starts_with(fs::canonicalize(root)?) || !canonical.is_file() {
                return Err(
                    "AVR package symlink escapes package tree or targets a directory".into(),
                );
            }
            Some(
                fs::read_link(&path)?
                    .to_str()
                    .ok_or("non-UTF8 AVR link")?
                    .to_owned(),
            )
        } else {
            None
        };
        let metadata = fs::metadata(&path)?;
        if !metadata.is_file() {
            return Err("unsupported AVR package entry".into());
        }
        *total = total
            .checked_add(metadata.len())
            .ok_or("AVR package size overflow")?;
        if *total > MAX_BYTES || files.len() >= MAX_FILES {
            return Err("AVR package inventory exceeds bound".into());
        }
        #[cfg(unix)]
        let executable = {
            use std::os::unix::fs::PermissionsExt;
            metadata.permissions().mode() & 0o111 != 0
        };
        #[cfg(not(unix))]
        let executable = false;
        files.insert(
            path.strip_prefix(root)?
                .to_str()
                .ok_or("non-UTF8 AVR path")?
                .replace('\\', "/"),
            Entry {
                sha256: sha256_file(&path)?,
                bytes: metadata.len(),
                executable,
                symlink: link,
            },
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;
