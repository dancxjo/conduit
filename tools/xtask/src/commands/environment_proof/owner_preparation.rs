//! Provision public owner trust, flash the actual Root and retain the Body owner
//! through signed native preparation. No Body start or sensor dispatch is claimed.
use super::{execute, host_target, software_ready};
#[path = "../esp32_flash_tool.rs"]
mod flash_tool;
use sha2::{Digest, Sha256};
use std::{error::Error, fs, path::Path, process::Command};
pub(super) fn provision_flash_tool(root: &Path) -> Result<std::path::PathBuf, Box<dyn Error>> {
    flash_tool::provision_espflash(root)
}
pub(super) fn owner_key_directory() -> Result<std::path::PathBuf, Box<dyn Error>> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|p| std::path::PathBuf::from(p).join(".local/share"))
        })
        .ok_or("host has no private data directory")?;
    let parent = data.join("conduit");
    fs::create_dir_all(&parent)?;
    Ok(parent.join("environment-owner"))
}
pub(super) fn prove(root: &Path, mode: &str) -> Result<(), Box<dyn Error>> {
    let native_face = mode == "run-face";
    let keys = owner_key_directory()?;
    execute(
        Command::new("cargo")
            .current_dir(root)
            .args([
                "build",
                "--locked",
                "--manifest-path",
                "bodies/environment/make/Cargo.toml",
                "--example",
                "native_owner",
                "--example",
                "native_owner_key",
                "--example",
                "native_admission",
            ])
            .env("CARGO_TARGET_DIR", host_target(root)),
    )?;
    execute(Command::new(host_target(root).join("debug/examples/native_owner_key")).arg(&keys))?;
    let package = root.join("targets/esp32/proof/tm1637");
    let toolchain: toml::Value =
        toml::from_str(&fs::read_to_string(package.join("rust-toolchain.toml"))?)?;
    let channel = toolchain["toolchain"]["channel"]
        .as_str()
        .ok_or("pinned ESP32 toolchain")?;
    execute(
        Command::new("cargo")
            .current_dir(package)
            .arg(format!("+{channel}"))
            .args([
                "build",
                "--locked",
                "--release",
                "--features",
                if native_face {
                    "native-face-root"
                } else {
                    "native-root"
                },
            ])
            .env("CARGO_TARGET_DIR", root.join("target/environment-esp32"))
            .env(
                "CONDUIT_ENVIRONMENT_OWNER_PUBLIC_KEY_FILE",
                keys.join("owner.pub"),
            ),
    )?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let image = root.join(format!("target/environment-owner-images/{stamp}"));
    fs::create_dir_all(&image)?;
    let elf = image.join("root.elf");
    fs::copy(
        root.join(
            "target/environment-esp32/xtensa-esp32-none-elf/release/conduit-esp32-tm1637-probe",
        ),
        &elf,
    )?;
    let source = Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "HEAD"])
        .output()?;
    let dirty = Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain", "--untracked-files=no"])
        .output()?;
    if !source.status.success() || !dirty.status.success() {
        return Err("source identity unavailable".into());
    }
    let image_receipt = serde_json::json!({
        "schema":"conduit.environment/owner-root-image@1", "artifact":elf,
        "elf_sha256":format!("{:x}", Sha256::digest(fs::read(&elf)?)),
        "owner_public_key_sha256":format!("{:x}", Sha256::digest(fs::read(keys.join("owner.pub"))?)),
        "source_head":String::from_utf8(source.stdout)?.trim(),
        "source_dirty":!dirty.stdout.is_empty(), "body_started":false,
        "profile":if native_face { "native-face-root" } else { "native-root" },
    });
    fs::write(
        image.join("image-receipt.json"),
        serde_json::to_vec_pretty(&image_receipt)?,
    )?;
    let tool = flash_tool::provision_espflash(root)?;
    execute(
        Command::new(tool)
            .args([
                "flash",
                "--chip",
                "esp32",
                "--port",
                "/dev/ttyUSB0",
                "--baud",
                "115200",
                "--non-interactive",
                "--skip-update-check",
            ])
            .arg(&elf),
    )?;
    let admission = software_ready::capture_ready_body_admission(root)?;
    let capture = software_ready::capture_and_plan_with_body(
        root,
        Some(&admission),
        if native_face {
            "native-face-root"
        } else {
            "native-root"
        },
    )?;
    execute(
        Command::new("python3")
            .current_dir(root)
            .arg("bodies/environment/tools/native-assignment-ingress.py")
            .arg("--capture")
            .arg(&capture),
    )?;
    fs::write(
        capture.join("owner-root-image.json"),
        serde_json::to_vec_pretty(&image_receipt)?,
    )?;
    let mut observation = Command::new("python3");
    observation
        .current_dir(root)
        .arg("bodies/environment/tools/native-owner-preparation.py")
        .arg("--capture")
        .arg(capture)
        .arg("--admission")
        .arg(admission)
        .arg("--keys")
        .arg(keys)
        .arg("--mode")
        .arg(if native_face { "run" } else { mode })
        .arg("--owner")
        .arg(host_target(root).join("debug/examples/native_owner"));
    if native_face {
        observation.env("CONDUIT_ENVIRONMENT_REQUIRE_NATIVE_FACE", "1");
    }
    execute(&mut observation)
}
